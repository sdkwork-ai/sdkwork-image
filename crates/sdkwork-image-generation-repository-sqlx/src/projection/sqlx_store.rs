use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_database_sqlx::DatabasePool;
use sdkwork_image_generation_workflow_service::{
    ImageGenerationInputSnapshot, ImageGenerationOutputPersistenceRow,
    ImageGenerationPersistencePlan, ImageProviderRequestSnapshot,
};
use sdkwork_utils_rust::uuid;
use serde::{Deserialize, Serialize};

use super::lifecycle::{apply_insert_lifecycle_side_effects, apply_refresh_lifecycle_side_effects};
use super::scope::{actor_user_id, organization_id, parse_scope_id};
use super::{GenerationProjectionRecord, GenerationProjectionRepository};
use crate::RepositoryError;

#[derive(Clone)]
pub struct SqlxGenerationProjectionRepository {
    pool: DatabasePool,
}

impl SqlxGenerationProjectionRepository {
    pub fn new(pool: DatabasePool) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct ProviderStateSnapshot {
    provider_request: ImageProviderRequestSnapshot,
}

#[derive(Debug, Serialize, Deserialize)]
struct JobMetadataSnapshot {
    api_wire: serde_json::Value,
}

#[async_trait]
impl GenerationProjectionRepository for SqlxGenerationProjectionRepository {
    async fn insert(&self, record: GenerationProjectionRecord) -> Result<(), RepositoryError> {
        let tenant_id = parse_scope_id(&record.scope.tenant_id, "tenant_id")?;
        let organization_id = organization_id(&record.scope)?;
        let user_id = actor_user_id(&record.scope.actor)?;
        let input =
            record.persistence.input_snapshot.as_ref().ok_or_else(|| {
                RepositoryError::Validation("input snapshot is required".to_string())
            })?;
        let provider_state = ProviderStateSnapshot {
            provider_request: record.provider_request.clone(),
        };
        let metadata = JobMetadataSnapshot {
            api_wire: record.wire_json.clone(),
        };
        let input_snapshot_json = serde_json::to_value(input)?;
        let provider_state_json = serde_json::to_value(&provider_state)?;
        let metadata_json = serde_json::to_value(&metadata)?;
        let resolution = input
            .resolution
            .clone()
            .unwrap_or_else(|| "1024x1024".to_string());
        let style = input.style.clone().unwrap_or_else(|| "default".to_string());

        match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_generation_job");
                let mut tx = pool.begin().await?;
                sqlx::query(&format!(
                    r#"
INSERT INTO {table} (
    uuid, tenant_id, organization_id, user_id, prompt, negative_prompt,
    resolution, style, model_id, provider_id, scene, provider_code, provider_operation,
    idempotency_key, callback_url, job_status, visibility, input_snapshot, provider_state,
    metadata, drive_sync_status, output_asset_count, provider_task_id, provider_status,
    drive_space_id, queued_at
) VALUES (
    $1, $2, $3, $4, $5, $6,
    $7, $8, $9, $10, $11, $12, $13,
    $14, $15, $16, $17, $18, $19,
    $20, $21, $22, $23, $24,
    $25, CURRENT_TIMESTAMP
)
"#
                ))
                .bind(&record.generation_id)
                .bind(tenant_id)
                .bind(organization_id)
                .bind(user_id)
                .bind(&input.prompt)
                .bind(&input.negative_prompt)
                .bind(&resolution)
                .bind(&style)
                .bind(&input.model)
                .bind(&input.provider_code)
                .bind(&input.scene)
                .bind(&record.persistence.provider_code)
                .bind(&input.provider_operation)
                .bind(&input.idempotency_key)
                .bind(&input.callback_url)
                .bind(record.persistence.job_status_code)
                .bind(1_i32)
                .bind(input_snapshot_json)
                .bind(&provider_state_json)
                .bind(metadata_json)
                .bind(&record.persistence.drive_sync_status)
                .bind(record.persistence.output_rows.len() as i32)
                .bind(&record.persistence.provider_task_id)
                .bind(&record.persistence.provider_status)
                .bind(
                    record
                        .persistence
                        .output_rows
                        .first()
                        .map(|row| row.drive_space_id.as_str()),
                )
                .execute(&mut *tx)
                .await?;

                let job_id = fetch_job_id_pg(
                    &mut tx,
                    &table,
                    tenant_id,
                    organization_id,
                    &record.generation_id,
                )
                .await?;
                upsert_outputs_pg(
                    &mut tx,
                    ctx,
                    tenant_id,
                    organization_id,
                    user_id,
                    job_id,
                    &record.generation_id,
                    &record.persistence,
                )
                .await?;
                apply_insert_lifecycle_side_effects(
                    &self.pool,
                    ctx,
                    tenant_id,
                    organization_id,
                    job_id,
                    &record.generation_id,
                    &record.persistence,
                    provider_state_json.clone(),
                    input,
                    &record.provider_request,
                )
                .await?;
                tx.commit().await?;
            }
        }

        Ok(())
    }

    async fn get(
        &self,
        tenant_id: &str,
        generation_id: &str,
    ) -> Result<Option<GenerationProjectionRecord>, RepositoryError> {
        let tenant_id = parse_scope_id(tenant_id, "tenant_id")?;
        let row = match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                read_job_row_pg(pool, ctx, tenant_id, generation_id).await?
            }
        };
        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(row.into_record(tenant_id)?))
    }

    async fn list_wire_json(
        &self,
        tenant_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<serde_json::Value>, RepositoryError> {
        let tenant_id = parse_scope_id(tenant_id, "tenant_id")?;
        let limit = limit.clamp(1, 100);
        let offset = offset.max(0);
        match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                list_wire_json_pg(pool, ctx, tenant_id, limit, offset).await
            }
        }
    }

    async fn update_after_refresh(
        &self,
        tenant_id: &str,
        organization_id: Option<&str>,
        generation_id: &str,
        wire_json: serde_json::Value,
        persistence: &ImageGenerationPersistencePlan,
    ) -> Result<(), RepositoryError> {
        let tenant_id = parse_scope_id(tenant_id, "tenant_id")?;
        let organization_id = match organization_id {
            Some(value) if !value.trim().is_empty() => parse_scope_id(value, "organization_id")?,
            _ => 0,
        };
        let metadata = JobMetadataSnapshot {
            api_wire: wire_json,
        };
        let metadata_json = serde_json::to_value(&metadata)?;
        let drive_space_id = persistence
            .output_rows
            .first()
            .map(|row| row.drive_space_id.as_str());
        match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_generation_job");
                let mut tx = pool.begin().await?;
                let result = sqlx::query(&format!(
                    r#"
UPDATE {table}
SET job_status = $1,
    drive_sync_status = $2,
    provider_task_id = $3,
    provider_status = $4,
    output_asset_count = $5,
    metadata = $6,
    drive_space_id = COALESCE($7, drive_space_id),
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $8
  AND organization_id = $9
  AND uuid = $10
"#
                ))
                .bind(persistence.job_status_code)
                .bind(&persistence.drive_sync_status)
                .bind(&persistence.provider_task_id)
                .bind(&persistence.provider_status)
                .bind(persistence.output_rows.len() as i32)
                .bind(metadata_json)
                .bind(drive_space_id)
                .bind(tenant_id)
                .bind(organization_id)
                .bind(generation_id)
                .execute(&mut *tx)
                .await?;
                if result.rows_affected() == 0 {
                    return Err(RepositoryError::NotFound);
                }
                if !persistence.output_rows.is_empty() {
                    let (job_id, user_id) = fetch_job_identity_pg(
                        &mut tx,
                        &table,
                        tenant_id,
                        organization_id,
                        generation_id,
                    )
                    .await?;
                    upsert_outputs_pg(
                        &mut tx,
                        ctx,
                        tenant_id,
                        organization_id,
                        user_id,
                        job_id,
                        generation_id,
                        persistence,
                    )
                    .await?;
                }
                if persistence
                    .repository_methods
                    .iter()
                    .any(|method| method == "mark_generation_succeeded")
                {
                    sqlx::query(&format!(
                        r#"
UPDATE {table}
SET job_status = $1,
    drive_sync_status = 'imported',
    finished_at = CURRENT_TIMESTAMP,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $2
  AND organization_id = $3
  AND uuid = $4
"#
                    ))
                    .bind(persistence.job_status_code)
                    .bind(tenant_id)
                    .bind(organization_id)
                    .bind(generation_id)
                    .execute(&mut *tx)
                    .await?;
                }
                let (job_id, _) = fetch_job_identity_pg(
                    &mut tx,
                    &table,
                    tenant_id,
                    organization_id,
                    generation_id,
                )
                .await?;
                let (input_snapshot, provider_request) = fetch_job_lifecycle_snapshots_pg(
                    &mut tx,
                    &table,
                    tenant_id,
                    organization_id,
                    generation_id,
                )
                .await?;
                apply_refresh_lifecycle_side_effects(
                    &self.pool,
                    ctx,
                    tenant_id,
                    organization_id,
                    job_id,
                    generation_id,
                    persistence,
                    &input_snapshot,
                    &provider_request,
                )
                .await?;
                tx.commit().await?;
            }
        }
        Ok(())
    }

    async fn cancel_generation(
        &self,
        tenant_id: &str,
        organization_id: Option<&str>,
        generation_id: &str,
        wire_json: serde_json::Value,
    ) -> Result<(), RepositoryError> {
        let tenant_id = parse_scope_id(tenant_id, "tenant_id")?;
        let organization_id = match organization_id {
            Some(value) if !value.trim().is_empty() => parse_scope_id(value, "organization_id")?,
            _ => 0,
        };
        let metadata = JobMetadataSnapshot {
            api_wire: wire_json,
        };
        let metadata_json = serde_json::to_value(&metadata)?;
        let affected = match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_generation_job");
                sqlx::query(&format!(
                    r#"
UPDATE {table}
SET metadata = $1,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $2
  AND organization_id = $3
  AND uuid = $4
  AND deleted_at IS NULL
  AND job_status IN (1, 2)
"#
                ))
                .bind(metadata_json)
                .bind(tenant_id)
                .bind(organization_id)
                .bind(generation_id)
                .execute(pool)
                .await?
                .rows_affected()
            }
        };
        if affected > 0 {
            return Ok(());
        }
        let exists = match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_generation_job");
                sqlx::query_scalar::<_, i64>(&format!(
                    r#"SELECT COUNT(1) FROM {table} WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3 AND deleted_at IS NULL"#
                ))
                .bind(tenant_id)
                .bind(organization_id)
                .bind(generation_id)
                .fetch_one(pool)
                .await?
            }
        };
        if exists == 0 {
            Err(RepositoryError::NotFound)
        } else {
            Err(RepositoryError::Conflict(
                "generation cannot be cancelled in the current status".to_string(),
            ))
        }
    }
}

struct JobRow {
    organization_id: i64,
    user_id: Option<i64>,
    scene: Option<String>,
    provider_code: Option<String>,
    provider_state: serde_json::Value,
    metadata: serde_json::Value,
    input_snapshot: serde_json::Value,
    job_status: i32,
    drive_sync_status: String,
    provider_task_id: Option<String>,
    provider_status: Option<String>,
    generation_uuid: String,
}

impl JobRow {
    fn into_record(self, tenant_id: i64) -> Result<GenerationProjectionRecord, RepositoryError> {
        let provider_state: ProviderStateSnapshot = serde_json::from_value(self.provider_state)?;
        let metadata: JobMetadataSnapshot = serde_json::from_value(self.metadata)?;
        let input_snapshot: sdkwork_image_generation_workflow_service::ImageGenerationInputSnapshot =
            serde_json::from_value(self.input_snapshot)?;
        let _scene = self.scene;
        let persistence = ImageGenerationPersistencePlan {
            generation_id: self.generation_uuid.clone(),
            runtime_status:
                sdkwork_image_generation_service::ImageGenerationRuntimeStatus::Importing,
            job_status_code: self.job_status,
            drive_sync_status: self.drive_sync_status,
            provider_code: self.provider_code.clone().unwrap_or_default(),
            provider_task_id: self.provider_task_id.clone(),
            provider_status: self.provider_status.clone(),
            input_snapshot: Some(input_snapshot),
            provider_request_snapshot: Some(provider_state.provider_request.clone()),
            output_rows: Vec::new(),
            repository_methods: Vec::new(),
            outbox_events: Vec::new(),
        };
        let actor = match self.user_id {
            Some(user_id) if user_id > 0 => {
                sdkwork_image_generation_service::ImageGenerationActor::User {
                    user_id: user_id.to_string(),
                }
            }
            _ => sdkwork_image_generation_service::ImageGenerationActor::Anonymous {
                anonymous_id: "anonymous".to_string(),
            },
        };
        Ok(GenerationProjectionRecord {
            scope: sdkwork_image_generation_workflow_service::ImageGenerationScope {
                tenant_id: tenant_id.to_string(),
                organization_id: if self.organization_id == 0 {
                    None
                } else {
                    Some(self.organization_id.to_string())
                },
                actor,
            },
            generation_id: self.generation_uuid,
            persistence,
            provider_request: provider_state.provider_request,
            wire_json: metadata.api_wire,
        })
    }
}

async fn fetch_job_id_pg(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: &str,
    tenant_id: i64,
    organization_id: i64,
    generation_id: &str,
) -> Result<i64, RepositoryError> {
    Ok(
        fetch_job_identity_pg(tx, table, tenant_id, organization_id, generation_id)
            .await?
            .0,
    )
}

async fn fetch_job_identity_pg(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: &str,
    tenant_id: i64,
    organization_id: i64,
    generation_id: &str,
) -> Result<(i64, i64), RepositoryError> {
    let row: (i64, Option<i64>) = sqlx::query_as(&format!(
        r#"SELECT id, user_id FROM {table} WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3"#
    ))
    .bind(tenant_id)
    .bind(organization_id)
    .bind(generation_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok((row.0, row.1.unwrap_or(0)))
}

async fn upsert_outputs_pg(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ctx: &sdkwork_database_sqlx::PoolContext,
    tenant_id: i64,
    organization_id: i64,
    user_id: i64,
    job_id: i64,
    generation_uuid: &str,
    persistence: &ImageGenerationPersistencePlan,
) -> Result<(), RepositoryError> {
    let table = ctx.table_name("image_generation_output");
    for row in &persistence.output_rows {
        insert_output_pg(
            tx,
            &table,
            tenant_id,
            organization_id,
            user_id,
            job_id,
            generation_uuid,
            row,
        )
        .await?;
    }
    Ok(())
}

async fn insert_output_pg(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: &str,
    tenant_id: i64,
    organization_id: i64,
    user_id: i64,
    job_id: i64,
    generation_uuid: &str,
    row: &ImageGenerationOutputPersistenceRow,
) -> Result<(), RepositoryError> {
    let output_uuid = uuid();
    let size_bytes = row
        .size_bytes
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok());
    sqlx::query(&format!(
        r#"
INSERT INTO {table} (
    uuid, tenant_id, organization_id, user_id, generation_job_id, generation_uuid,
    output_index, media_kind, scene, provider_code, provider_operation, provider_task_id,
    provider_asset_id, provider_uri, provider_url, drive_space_type, drive_space_id,
    drive_parent_node_id, drive_node_id, drive_uri, file_name, mime_type, size_bytes,
    width, height, duration_seconds, sync_status
) VALUES (
    $1, $2, $3, $4, $5, $6,
    $7, $8, $9, $10, $11, $12,
    $13, $14, $15, $16, $17,
    $18, $19, $20, $21, $22, $23,
    $24, $25, $26, $27
)
ON CONFLICT (tenant_id, organization_id, generation_job_id, output_index)
DO UPDATE SET
    scene = EXCLUDED.scene,
    provider_code = EXCLUDED.provider_code,
    provider_task_id = EXCLUDED.provider_task_id,
    provider_asset_id = EXCLUDED.provider_asset_id,
    provider_uri = EXCLUDED.provider_uri,
    provider_url = EXCLUDED.provider_url,
    drive_space_id = EXCLUDED.drive_space_id,
    drive_node_id = EXCLUDED.drive_node_id,
    drive_uri = EXCLUDED.drive_uri,
    sync_status = EXCLUDED.sync_status,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
"#
    ))
    .bind(output_uuid)
    .bind(tenant_id)
    .bind(organization_id)
    .bind(user_id)
    .bind(job_id)
    .bind(generation_uuid)
    .bind(row.output_index)
    .bind(&row.media_kind)
    .bind(&row.scene)
    .bind(&row.provider_code)
    .bind::<Option<String>>(None)
    .bind::<Option<String>>(None)
    .bind(&row.provider_asset_id)
    .bind(&row.provider_uri)
    .bind(&row.provider_url)
    .bind(&row.drive_space_type)
    .bind(&row.drive_space_id)
    .bind(&row.drive_parent_node_id)
    .bind(&row.drive_node_id)
    .bind(&row.drive_uri)
    .bind(&row.file_name)
    .bind(&row.mime_type)
    .bind(size_bytes)
    .bind(row.width)
    .bind(row.height)
    .bind(row.duration_seconds)
    .bind(&row.sync_status)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn read_job_row_pg(
    pool: &sqlx::PgPool,
    ctx: &sdkwork_database_sqlx::PoolContext,
    tenant_id: i64,
    generation_id: &str,
) -> Result<Option<JobRow>, RepositoryError> {
    let table = ctx.table_name("image_generation_job");
    let row = sqlx::query_as::<
        _,
        (
            i64,
            Option<i64>,
            Option<String>,
            Option<String>,
            serde_json::Value,
            serde_json::Value,
            serde_json::Value,
            i32,
            String,
            Option<String>,
            Option<String>,
            String,
        ),
    >(&format!(
        r#"
SELECT organization_id, user_id, scene, provider_code, provider_state, metadata, input_snapshot,
       job_status, drive_sync_status, provider_task_id, provider_status, uuid
FROM {table}
WHERE tenant_id = $1 AND uuid = $2 AND deleted_at IS NULL
"#
    ))
    .bind(tenant_id)
    .bind(generation_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(
        |(
            organization_id,
            user_id,
            scene,
            provider_code,
            provider_state,
            metadata,
            input_snapshot,
            job_status,
            drive_sync_status,
            provider_task_id,
            provider_status,
            generation_uuid,
        )| JobRow {
            organization_id,
            user_id,
            scene,
            provider_code,
            provider_state,
            metadata,
            input_snapshot,
            job_status,
            drive_sync_status,
            provider_task_id,
            provider_status,
            generation_uuid,
        },
    ))
}

async fn list_wire_json_pg(
    pool: &sqlx::PgPool,
    ctx: &sdkwork_database_sqlx::PoolContext,
    tenant_id: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<serde_json::Value>, RepositoryError> {
    let table = ctx.table_name("image_generation_job");
    let rows: Vec<(serde_json::Value,)> = sqlx::query_as(&format!(
        r#"
SELECT metadata FROM {table}
WHERE tenant_id = $1 AND deleted_at IS NULL
ORDER BY id DESC
LIMIT $2 OFFSET $3
"#
    ))
    .bind(tenant_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;
    rows.into_iter()
        .map(|(metadata,)| {
            let snapshot: JobMetadataSnapshot = serde_json::from_value(metadata)?;
            Ok(snapshot.api_wire)
        })
        .collect()
}

async fn fetch_job_lifecycle_snapshots_pg(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    table: &str,
    tenant_id: i64,
    organization_id: i64,
    generation_id: &str,
) -> Result<(ImageGenerationInputSnapshot, ImageProviderRequestSnapshot), RepositoryError> {
    let row: (serde_json::Value, serde_json::Value) = sqlx::query_as(&format!(
        r#"SELECT input_snapshot, provider_state FROM {table} WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3"#
    ))
    .bind(tenant_id)
    .bind(organization_id)
    .bind(generation_id)
    .fetch_one(&mut **tx)
    .await?;
    let input_snapshot: ImageGenerationInputSnapshot = serde_json::from_value(row.0)?;
    let provider_state: ProviderStateSnapshot = serde_json::from_value(row.1)?;
    Ok((input_snapshot, provider_state.provider_request))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sdkwork_image_generation_workflow_service::rehydrate_image_provider_dispatch_plan;

    #[test]
    fn provider_state_round_trips_snapshot() {
        let snapshot = ProviderStateSnapshot {
            provider_request: ImageProviderRequestSnapshot {
                provider_id: "sdkwork-image-generation-provider-adapter".to_string(),
                provider_code: "openai".to_string(),
                provider_operation: "openai.images.generate".to_string(),
                task_mode: "sync".to_string(),
                prompt: "hero".to_string(),
                negative_prompt: None,
                model: None,
                size: None,
                quality: None,
                output_count: 1,
                output_count_provider_parameter: Some("n".to_string()),
                reference_images: Vec::new(),
                callback_url: None,
                idempotency_key: None,
            },
        };
        let json = serde_json::to_value(&snapshot).expect("json");
        let restored: ProviderStateSnapshot = serde_json::from_value(json).expect("restore");
        assert_eq!(restored.provider_request.provider_code, "openai");
        rehydrate_image_provider_dispatch_plan(&restored.provider_request).expect("rehydrate");
    }
}
