use sdkwork_assets_bridge_image::{
    drive_ingest_context_from_image_actor, plan_unified_assets_drive_import_from_image,
};
use sdkwork_assets_ingestion::DriveImportPlan;
use sdkwork_assets_ingestion_drive::{
    build_prepare_commands_for_import_plan, build_upload_bytes_command,
    DriveImportExecutionContext, DriveUploaderContext,
};
use sdkwork_drive_storage_contract::DriveObjectStore;
use sdkwork_drive_workspace_service::ports::uploader_store::DriveUploaderStore;
use sdkwork_drive_workspace_service::uploader::DriveUploaderService;
use sdkwork_image_generation_service::{
    DriveGeneratedMediaContext, DriveGeneratedMediaImportPlan, GeneratedMediaOutput,
    ImageGenerationActor, IMAGE_WORKSPACE,
};

use crate::provider_fetch::{ProviderArtifactFetcher, ProviderArtifactRef};
use crate::ImageRuntimeError;

#[derive(Clone, Debug)]
pub struct ImageDriveUploadPreparation {
    pub output_index: i32,
    pub provider_url: Option<String>,
    pub provider_uri: Option<String>,
    pub prepare: sdkwork_drive_workspace_service::uploader::PrepareUploaderUploadCommand,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompletedDriveImportArtifact {
    pub output_index: i32,
    pub drive_node_id: String,
    pub drive_uri: String,
    pub checksum_sha256_hex: String,
}

pub fn plan_drive_upload_preparations(
    tenant_id: impl Into<String>,
    organization_id: Option<String>,
    actor: &ImageGenerationActor,
    drive_import_plans: &[DriveGeneratedMediaImportPlan],
    outputs: &[GeneratedMediaOutput],
    operation_type: impl Into<String>,
    operator_id: impl Into<String>,
    now_epoch_ms: i64,
) -> Result<(DriveImportPlan, Vec<ImageDriveUploadPreparation>), ImageRuntimeError> {
    if drive_import_plans.is_empty() {
        return Ok((empty_import_plan(), Vec::new()));
    }
    let context = DriveGeneratedMediaContext {
        tenant_id: tenant_id.into(),
        organization_id,
        generation_id: drive_import_plans[0].generation_id.clone(),
        provider_code: drive_import_plans[0].provider_code.clone(),
        model: None,
        scene: drive_import_plans[0].scene.clone(),
        actor: actor.clone(),
    };
    let assets_plan =
        plan_unified_assets_drive_import_from_image(&context, operation_type, outputs)
            .map_err(|_| ImageRuntimeError::Planning("unified assets drive import plan failed"))?;
    let execution = build_drive_import_execution_context(
        context.tenant_id.clone(),
        context.organization_id.clone(),
        actor,
        operator_id.into(),
        now_epoch_ms,
    )?;
    let prepares = build_prepare_commands_for_import_plan(&assets_plan, &execution)
        .map_err(|error| ImageRuntimeError::DriveImport(error.to_string()))?;
    let preparations = assets_plan
        .items
        .iter()
        .zip(prepares)
        .map(|(item, prepare)| ImageDriveUploadPreparation {
            output_index: item.output_index,
            provider_url: item.provider_url.clone(),
            provider_uri: item.provider_uri.clone(),
            prepare,
        })
        .collect();
    Ok((assets_plan, preparations))
}

pub fn build_drive_import_execution_context(
    tenant_id: String,
    organization_id: Option<String>,
    actor: &ImageGenerationActor,
    operator_id: String,
    now_epoch_ms: i64,
) -> Result<DriveImportExecutionContext, ImageRuntimeError> {
    let ingest = drive_ingest_context_from_image_actor(tenant_id.clone(), actor)
        .map_err(|_| ImageRuntimeError::Planning("drive ingest context failed"))?;
    Ok(DriveImportExecutionContext {
        uploader: DriveUploaderContext {
            tenant_id,
            organization_id,
            operator_id,
            now_epoch_ms,
            app_id: IMAGE_WORKSPACE.to_string(),
            app_resource_type: "ai_generation_output".to_string(),
        },
        ingest,
    })
}

pub async fn execute_drive_import_uploads<S, O, F>(
    uploader: &DriveUploaderService<S>,
    object_store: &O,
    fetcher: &F,
    import_plan: &DriveImportPlan,
    preparations: &[ImageDriveUploadPreparation],
    execution: &DriveImportExecutionContext,
) -> Result<Vec<CompletedDriveImportArtifact>, ImageRuntimeError>
where
    S: DriveUploaderStore,
    O: DriveObjectStore,
    F: ProviderArtifactFetcher,
{
    if preparations.is_empty() {
        return Ok(Vec::new());
    }
    if preparations.len() != import_plan.items.len() {
        return Err(ImageRuntimeError::Validation(
            "drive import preparations must match import plan items",
        ));
    }

    let mut completed = Vec::with_capacity(preparations.len());
    for (item, preparation) in import_plan.items.iter().zip(preparations) {
        if item.output_index != preparation.output_index {
            return Err(ImageRuntimeError::Validation(
                "drive import preparation output_index mismatch",
            ));
        }
        let content = fetcher
            .fetch_provider_artifact(&ProviderArtifactRef {
                output_index: preparation.output_index,
                provider_url: preparation.provider_url.clone(),
                provider_uri: preparation.provider_uri.clone(),
                file_name: item.media_resource.file_name.clone(),
                mime_type: item.media_resource.mime_type.clone(),
            })
            .await
            .map_err(ImageRuntimeError::ProviderFetch)?;
        let upload = build_upload_bytes_command(item, import_plan, execution, content.body)
            .map_err(|error| ImageRuntimeError::DriveImport(format!("{error:?}")))?;
        let uploaded = uploader
            .upload_bytes(object_store, upload)
            .await
            .map_err(|error| ImageRuntimeError::DriveImport(format!("{error:?}")))?;
        completed.push(CompletedDriveImportArtifact {
            output_index: preparation.output_index,
            drive_node_id: uploaded.node_id.clone(),
            drive_uri: format!(
                "drive://spaces/{}/nodes/{}",
                uploaded.space_id, uploaded.node_id
            ),
            checksum_sha256_hex: uploaded.checksum_sha256_hex.unwrap_or_default(),
        });
    }
    Ok(completed)
}

fn empty_import_plan() -> DriveImportPlan {
    DriveImportPlan {
        generation_id: String::new(),
        provider_code: String::new(),
        items: Vec::new(),
    }
}
