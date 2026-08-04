use std::sync::Arc;

use sdkwork_assets_ingestion::DriveImportPlan;
use sdkwork_image_generation_repository_sqlx::{
    GenerationProjectionRecord, GenerationProjectionRepository, RepositoryError,
};
use sdkwork_image_generation_runtime_service::{
    execute_create_generation_dispatch, execute_refresh_generation_dispatch,
    CompletedDriveImportArtifact, ImageDriveImportRuntime, ImageGenerationCreateRuntimeInput,
    ImageGenerationRefreshRuntimeInput,
};
use sdkwork_image_generation_service::{
    ImageGenerationActor, ImageGenerationCreateCommand, ImageGenerationRuntimeStatus,
    ImageGenerationService as ProviderGenerationService,
};
use sdkwork_image_generation_workflow_service::{
    finalize_persistence_after_drive_import, plan_generation_create_persistence_plan_with_dispatch,
    plan_generation_refresh_persistence_plan, ImageGenerationScope, OutputDriveImportState,
};
use sdkwork_utils_rust::uuid;

use crate::subject::RuntimeSubject;
use crate::wire::{
    ImageGenerationCommandWire, ImageGenerationOutputWire, ImageGenerationRefreshCommandWire,
    ImageGenerationWire,
};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImageGenerationServiceError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("generation not found")]
    NotFound,
    #[error("provider dispatch failed: {0}")]
    Dispatch(String),
    #[error("planning failed: {0}")]
    Planning(String),
    #[error("persistence failed: {0}")]
    Persistence(String),
    #[error("drive import failed: {0}")]
    DriveImport(String),
    #[error("conflict: {0}")]
    Conflict(String),
}

pub struct ImageGenerationService {
    provider_service: Arc<ProviderGenerationService>,
    store: Arc<dyn GenerationProjectionRepository>,
    drive_import: Option<Arc<ImageDriveImportRuntime>>,
}

impl ImageGenerationService {
    pub fn new(
        provider_service: Arc<ProviderGenerationService>,
        store: Arc<dyn GenerationProjectionRepository>,
        drive_import: Option<Arc<ImageDriveImportRuntime>>,
    ) -> Self {
        Self {
            provider_service,
            store,
            drive_import,
        }
    }

    pub async fn create_generation(
        &self,
        subject: &RuntimeSubject,
        command: ImageGenerationCommandWire,
    ) -> Result<ImageGenerationWire, ImageGenerationServiceError> {
        let domain_command = map_create_command(command)?;
        let generation_id = uuid();
        let now_epoch_ms = current_epoch_ms();
        let scope = runtime_scope(subject);
        let runtime = execute_create_generation_dispatch(
            self.provider_service.as_ref(),
            ImageGenerationCreateRuntimeInput {
                scope: scope.clone(),
                generation_id: generation_id.clone(),
                command: domain_command.clone(),
                operator_id: subject.user_id.clone(),
                now_epoch_ms,
            },
        )
        .await
        .map_err(|error| ImageGenerationServiceError::Dispatch(error.to_string()))?;
        let mut wire = map_generation_wire(&generation_id, &runtime)?;
        maybe_execute_drive_imports(
            self.drive_import.as_deref(),
            &scope,
            &subject.user_id,
            now_epoch_ms,
            runtime.provider_result.ready_for_drive_import,
            &runtime.import_plan,
            &runtime.upload_preparations,
            &mut wire,
        )
        .await?;
        let mut persistence = plan_generation_create_persistence_plan_with_dispatch(
            scope.clone(),
            generation_id.clone(),
            domain_command,
            runtime.dispatch_plan.clone(),
            Some(runtime.provider_result.clone()),
        )
        .map_err(|message| ImageGenerationServiceError::Planning(message.to_string()))?;
        sync_persistence_from_wire(&mut persistence, &wire);
        let provider_request = persistence
            .provider_request_snapshot
            .clone()
            .ok_or_else(|| {
                ImageGenerationServiceError::Planning(
                    "provider request snapshot is required".to_string(),
                )
            })?;
        let wire_json = serde_json::to_value(&wire)
            .map_err(|error| ImageGenerationServiceError::Persistence(error.to_string()))?;
        self.store
            .insert(GenerationProjectionRecord {
                scope,
                generation_id,
                persistence,
                provider_request,
                wire_json,
            })
            .await
            .map_err(map_repository_error)?;
        Ok(wire)
    }

    pub async fn cancel_generation(
        &self,
        subject: &RuntimeSubject,
        generation_id: &str,
        reason: Option<String>,
    ) -> Result<ImageGenerationWire, ImageGenerationServiceError> {
        let wire = self.get_generation(subject, generation_id).await?;
        if matches!(
            wire.status.as_str(),
            "succeeded" | "failed" | "cancelled" | "expired"
        ) {
            return Err(ImageGenerationServiceError::Conflict(format!(
                "generation cannot be cancelled in status {}",
                wire.status
            )));
        }
        let mut updated = wire;
        updated.status = ImageGenerationRuntimeStatus::CancelRequested
            .as_str()
            .to_string();
        if let Some(reason) = reason.filter(|value| !value.trim().is_empty()) {
            updated.metadata = Some(serde_json::json!({ "cancelReason": reason }));
        }
        let wire_json = serde_json::to_value(&updated)
            .map_err(|error| ImageGenerationServiceError::Persistence(error.to_string()))?;
        self.store
            .cancel_generation(
                &subject.tenant_id,
                subject.organization_id.as_deref(),
                generation_id.trim(),
                wire_json,
            )
            .await
            .map_err(map_repository_error)?;
        Ok(updated)
    }

    pub async fn get_generation(
        &self,
        subject: &RuntimeSubject,
        generation_id: &str,
    ) -> Result<ImageGenerationWire, ImageGenerationServiceError> {
        let record = self
            .store
            .get(&subject.tenant_id, generation_id.trim())
            .await
            .map_err(map_repository_error)?
            .ok_or(ImageGenerationServiceError::NotFound)?;
        decode_wire(record.wire_json)
    }

    pub async fn list_generations(
        &self,
        subject: &RuntimeSubject,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<ImageGenerationWire>, ImageGenerationServiceError> {
        let offset = (page.max(1) - 1) * page_size;
        let items = self
            .store
            .list_wire_json(&subject.tenant_id, page_size, offset)
            .await
            .map_err(map_repository_error)?;
        items.into_iter().map(decode_wire).collect()
    }

    pub async fn refresh_generation(
        &self,
        subject: &RuntimeSubject,
        generation_id: &str,
        command: ImageGenerationRefreshCommandWire,
    ) -> Result<ImageGenerationWire, ImageGenerationServiceError> {
        let stored = self
            .store
            .get(&subject.tenant_id, generation_id.trim())
            .await
            .map_err(map_repository_error)?
            .ok_or(ImageGenerationServiceError::NotFound)?;
        let dispatch_plan = stored
            .dispatch_plan()
            .map_err(|error| ImageGenerationServiceError::Persistence(error.to_string()))?;
        let wire: ImageGenerationWire = decode_wire(stored.wire_json.clone())?;
        let provider_task_id = command
            .provider_task_id
            .or(wire.provider_task_id.clone())
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                ImageGenerationServiceError::Validation(
                    "provider_task_id is required for refresh".to_string(),
                )
            })?;
        let scope = runtime_scope(subject);
        let runtime = execute_refresh_generation_dispatch(
            self.provider_service.as_ref(),
            ImageGenerationRefreshRuntimeInput {
                scope: scope.clone(),
                generation_id: generation_id.to_string(),
                scene: wire.scene.clone(),
                model: None,
                dispatch_plan,
                provider_task_id,
                operator_id: subject.user_id.clone(),
                now_epoch_ms: current_epoch_ms(),
            },
        )
        .await
        .map_err(|error| ImageGenerationServiceError::Dispatch(error.to_string()))?;
        let mut updated_wire = map_refresh_wire(generation_id, &wire, &runtime.provider_result)?;
        apply_import_plan_to_wire(&mut updated_wire, &runtime.import_plan);
        maybe_execute_drive_imports(
            self.drive_import.as_deref(),
            &scope,
            &subject.user_id,
            current_epoch_ms(),
            runtime.provider_result.ready_for_drive_import,
            &runtime.import_plan,
            &runtime.upload_preparations,
            &mut updated_wire,
        )
        .await?;
        let mut persistence = plan_generation_refresh_persistence_plan(
            scope,
            generation_id.to_string(),
            wire.scene.clone(),
            None,
            runtime.provider_result.clone(),
        )
        .map_err(|message| ImageGenerationServiceError::Planning(message.to_string()))?;
        sync_persistence_from_wire(&mut persistence, &updated_wire);
        let wire_json = serde_json::to_value(&updated_wire)
            .map_err(|error| ImageGenerationServiceError::Persistence(error.to_string()))?;
        self.store
            .update_after_refresh(
                &subject.tenant_id,
                subject.organization_id.as_deref(),
                generation_id.trim(),
                wire_json,
                &persistence,
            )
            .await
            .map_err(map_repository_error)?;
        Ok(updated_wire)
    }
}

fn map_repository_error(error: RepositoryError) -> ImageGenerationServiceError {
    match error {
        RepositoryError::NotFound => ImageGenerationServiceError::NotFound,
        RepositoryError::Conflict(message) => ImageGenerationServiceError::Conflict(message),
        RepositoryError::Validation(message) => ImageGenerationServiceError::Validation(message),
        RepositoryError::Database(message) | RepositoryError::Serialization(message) => {
            ImageGenerationServiceError::Persistence(message)
        }
    }
}

fn decode_wire(
    value: serde_json::Value,
) -> Result<ImageGenerationWire, ImageGenerationServiceError> {
    serde_json::from_value(value)
        .map_err(|error| ImageGenerationServiceError::Persistence(error.to_string()))
}

fn runtime_scope(subject: &RuntimeSubject) -> ImageGenerationScope {
    ImageGenerationScope {
        tenant_id: subject.tenant_id.clone(),
        organization_id: subject.organization_id.clone(),
        actor: ImageGenerationActor::User {
            user_id: subject.user_id.clone(),
        },
    }
}

fn map_create_command(
    command: ImageGenerationCommandWire,
) -> Result<ImageGenerationCreateCommand, ImageGenerationServiceError> {
    Ok(ImageGenerationCreateCommand {
        prompt: command.prompt,
        negative_prompt: command.negative_prompt,
        scene: command.scene,
        provider_code: command.provider_code,
        model: command.model,
        resolution: command.resolution,
        style: command.style,
        output_count: command.output_count,
        reference_images: command.reference_images,
        webhook_url: command.webhook_url,
        idempotency_key: command.idempotency_key,
    })
}

fn map_generation_wire(
    generation_id: &str,
    runtime: &sdkwork_image_generation_runtime_service::ImageGenerationCreateRuntimeResult,
) -> Result<ImageGenerationWire, ImageGenerationServiceError> {
    let record = &runtime.service_plan.record;
    let drive_import_plans = &runtime.service_plan.drive_import_plans;
    let outputs = runtime
        .provider_result
        .outputs
        .iter()
        .map(|output| {
            let import_plan = drive_import_plans
                .iter()
                .find(|plan| plan.output_index == output.output_index);
            ImageGenerationOutputWire {
                output_index: output.output_index,
                media_kind: output.kind.as_media_kind().to_string(),
                scene: record.scene.clone(),
                sync_status: "pending".to_string(),
                provider_code: Some(record.provider_code.clone()),
                provider_asset_id: output.provider_asset_id.clone(),
                provider_uri: output.provider_uri.clone(),
                provider_url: output.provider_url.clone(),
                drive_space_id: import_plan.map(|plan| plan.drive_space_id.clone()),
                drive_node_id: import_plan.map(|plan| plan.drive_node_id.clone()),
                drive_uri: import_plan.map(|plan| plan.drive_uri.clone()),
                file_name: output.file_name.clone(),
                mime_type: output.mime_type.clone(),
                size_bytes: output.size_bytes.map(|value| value.to_string()),
                width: output.width,
                height: output.height,
                duration_seconds: output.duration_seconds,
            }
        })
        .collect();
    Ok(ImageGenerationWire {
        generation_id: generation_id.to_string(),
        status: record.status.as_str().to_string(),
        scene: record.scene.clone(),
        provider_code: Some(record.provider_code.clone()),
        provider_task_id: record.provider_task_id.clone(),
        provider_status: record.provider_status.clone(),
        drive_space_id: record.drive_space_id.clone(),
        drive_sync_status: record.status.as_drive_sync_status().to_string(),
        output_asset_count: record.output_count,
        outputs,
        metadata: None,
    })
}

fn map_refresh_wire(
    generation_id: &str,
    wire: &ImageGenerationWire,
    result: &sdkwork_image_generation_service::NormalizedProviderGenerationResult,
) -> Result<ImageGenerationWire, ImageGenerationServiceError> {
    let mut updated = wire.clone();
    updated.status = result.status.as_str().to_string();
    updated.provider_task_id = result.provider_task_id.clone();
    updated.provider_status = result.provider_status.clone();
    updated.drive_sync_status = result.status.as_drive_sync_status().to_string();
    updated.output_asset_count = result.outputs.len() as i32;
    updated.outputs = result
        .outputs
        .iter()
        .map(|output| ImageGenerationOutputWire {
            output_index: output.output_index,
            media_kind: output.kind.as_media_kind().to_string(),
            scene: updated.scene.clone(),
            sync_status: "pending".to_string(),
            provider_code: Some(result.provider_code.clone()),
            provider_asset_id: output.provider_asset_id.clone(),
            provider_uri: output.provider_uri.clone(),
            provider_url: output.provider_url.clone(),
            drive_space_id: None,
            drive_node_id: None,
            drive_uri: None,
            file_name: output.file_name.clone(),
            mime_type: output.mime_type.clone(),
            size_bytes: output.size_bytes.map(|value| value.to_string()),
            width: output.width,
            height: output.height,
            duration_seconds: output.duration_seconds,
        })
        .collect();
    updated.generation_id = generation_id.to_string();
    if result.status == ImageGenerationRuntimeStatus::Failed {
        updated.drive_sync_status = "failed".to_string();
    }
    Ok(updated)
}

fn current_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

async fn maybe_execute_drive_imports(
    drive_import: Option<&ImageDriveImportRuntime>,
    scope: &ImageGenerationScope,
    operator_id: &str,
    now_epoch_ms: i64,
    ready_for_drive_import: bool,
    import_plan: &DriveImportPlan,
    upload_preparations: &[sdkwork_image_generation_runtime_service::ImageDriveUploadPreparation],
    wire: &mut ImageGenerationWire,
) -> Result<(), ImageGenerationServiceError> {
    if !ready_for_drive_import || upload_preparations.is_empty() {
        return Ok(());
    }
    let Some(runtime) = drive_import else {
        return Ok(());
    };
    let completed = runtime
        .execute_imports(
            scope.tenant_id.as_str(),
            scope.organization_id.as_deref(),
            &scope.actor,
            operator_id,
            now_epoch_ms,
            import_plan,
            upload_preparations,
        )
        .await
        .map_err(|error| ImageGenerationServiceError::DriveImport(error.to_string()))?;
    apply_completed_drive_imports(wire, &completed, import_plan);
    Ok(())
}

fn apply_import_plan_to_wire(wire: &mut ImageGenerationWire, import_plan: &DriveImportPlan) {
    for output in &mut wire.outputs {
        let Some(item) = import_plan
            .items
            .iter()
            .find(|item| item.output_index == output.output_index)
        else {
            continue;
        };
        output.drive_space_id = Some(item.drive_space_id.clone());
        output.drive_node_id = Some(item.drive_node_id.clone());
        output.drive_uri = Some(item.drive_uri.clone());
    }
    wire.drive_space_id = import_plan
        .items
        .first()
        .map(|item| item.drive_space_id.clone());
}

fn apply_completed_drive_imports(
    wire: &mut ImageGenerationWire,
    completed: &[CompletedDriveImportArtifact],
    import_plan: &DriveImportPlan,
) {
    for artifact in completed {
        let Some(output) = wire
            .outputs
            .iter_mut()
            .find(|output| output.output_index == artifact.output_index)
        else {
            continue;
        };
        output.sync_status = "imported".to_string();
        output.drive_node_id = Some(artifact.drive_node_id.clone());
        output.drive_uri = Some(artifact.drive_uri.clone());
        if let Some(item) = import_plan
            .items
            .iter()
            .find(|item| item.output_index == artifact.output_index)
        {
            output.drive_space_id = Some(item.drive_space_id.clone());
        }
    }
    if !wire.outputs.is_empty()
        && wire
            .outputs
            .iter()
            .all(|output| output.sync_status == "imported")
    {
        wire.drive_sync_status = "imported".to_string();
    }
    wire.drive_space_id = wire
        .outputs
        .first()
        .and_then(|output| output.drive_space_id.clone());
}

fn sync_persistence_from_wire(
    persistence: &mut sdkwork_image_generation_workflow_service::ImageGenerationPersistencePlan,
    wire: &ImageGenerationWire,
) {
    let outputs = wire
        .outputs
        .iter()
        .map(|output| OutputDriveImportState {
            output_index: output.output_index,
            sync_status: output.sync_status.clone(),
            drive_space_id: output.drive_space_id.clone(),
            drive_node_id: output.drive_node_id.clone(),
            drive_uri: output.drive_uri.clone(),
        })
        .collect::<Vec<_>>();
    finalize_persistence_after_drive_import(persistence, &wire.drive_sync_status, &outputs);
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_image_generation_repository_sqlx::InMemoryGenerationProjectionRepository;

    #[test]
    fn map_create_command_preserves_scene_and_prompt() {
        let command = map_create_command(ImageGenerationCommandWire {
            prompt: "hero".to_string(),
            negative_prompt: None,
            scene: "playground_image".to_string(),
            provider_code: Some("openai".to_string()),
            model: None,
            resolution: None,
            style: None,
            output_count: Some(1),
            reference_images: Vec::new(),
            webhook_url: None,
            idempotency_key: None,
        })
        .expect("command");
        assert_eq!(command.prompt, "hero");
        assert_eq!(command.scene, "playground_image");
    }

    #[test]
    fn service_accepts_injected_provider_service_and_in_memory_repository() {
        let client = cloudrouter_open_sdk::SdkworkAiClient::new(
            cloudrouter_open_sdk::SdkworkConfig::new("http://127.0.0.1:0"),
        )
        .expect("client");
        let provider = Arc::new(
            sdkwork_image_generation_provider_adapter::ImageGenerationProviderAdapter::new(client),
        );
        let registry = sdkwork_image_generation_service::ImageGenerationProviderRegistry::builder()
            .register(provider)
            .expect("provider")
            .default_provider(
                sdkwork_image_generation_provider_adapter::IMAGE_GENERATION_PROVIDER_ADAPTER_ID,
            )
            .build()
            .expect("registry");
        let _service = ImageGenerationService::new(
            Arc::new(ProviderGenerationService::new(registry)),
            InMemoryGenerationProjectionRepository::new(),
            None,
        );
    }
}
