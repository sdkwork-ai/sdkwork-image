use sdkwork_database_sqlx::{DatabasePool, PoolContext};
use sdkwork_image_generation_service::ImageGenerationRuntimeStatus;
use sdkwork_image_generation_workflow_service::{
    ImageGenerationInputSnapshot, ImageGenerationOutboxEvent, ImageGenerationPersistencePlan,
    ImageProviderRequestSnapshot,
};
use sdkwork_utils_rust::uuid;

use crate::RepositoryError;

pub(crate) async fn apply_insert_lifecycle_side_effects(
    pool: &DatabasePool,
    ctx: &PoolContext,
    tenant_id: i64,
    organization_id: i64,
    job_id: i64,
    generation_uuid: &str,
    persistence: &ImageGenerationPersistencePlan,
    provider_state_json: serde_json::Value,
    input_snapshot: &ImageGenerationInputSnapshot,
    provider_request: &ImageProviderRequestSnapshot,
) -> Result<(), RepositoryError> {
    apply_lifecycle_side_effects(
        pool,
        ctx,
        tenant_id,
        organization_id,
        job_id,
        generation_uuid,
        persistence,
        provider_state_json,
        input_snapshot,
        provider_request,
    )
    .await
}

pub(crate) async fn apply_refresh_lifecycle_side_effects(
    pool: &DatabasePool,
    ctx: &PoolContext,
    tenant_id: i64,
    organization_id: i64,
    job_id: i64,
    generation_uuid: &str,
    persistence: &ImageGenerationPersistencePlan,
    input_snapshot: &ImageGenerationInputSnapshot,
    provider_request: &ImageProviderRequestSnapshot,
) -> Result<(), RepositoryError> {
    apply_lifecycle_side_effects(
        pool,
        ctx,
        tenant_id,
        organization_id,
        job_id,
        generation_uuid,
        persistence,
        serde_json::json!({}),
        input_snapshot,
        provider_request,
    )
    .await
}

async fn apply_lifecycle_side_effects(
    pool: &DatabasePool,
    ctx: &PoolContext,
    tenant_id: i64,
    organization_id: i64,
    job_id: i64,
    generation_uuid: &str,
    persistence: &ImageGenerationPersistencePlan,
    provider_state_json: serde_json::Value,
    input_snapshot: &ImageGenerationInputSnapshot,
    provider_request: &ImageProviderRequestSnapshot,
) -> Result<(), RepositoryError> {
    if persistence
        .repository_methods
        .iter()
        .any(|method| method == "mark_provider_submitted")
    {
        mark_provider_submitted(
            pool,
            ctx,
            tenant_id,
            organization_id,
            generation_uuid,
            persistence,
            provider_state_json.clone(),
        )
        .await?;
    }
    if persistence
        .repository_methods
        .iter()
        .any(|method| method == "upsert_provider_task")
    {
        upsert_provider_task(
            pool,
            ctx,
            tenant_id,
            organization_id,
            job_id,
            persistence,
            input_snapshot,
            provider_request,
        )
        .await?;
    }
    if persistence
        .repository_methods
        .iter()
        .any(|method| method == "mark_generation_failed")
    {
        mark_generation_failed(
            pool,
            ctx,
            tenant_id,
            organization_id,
            generation_uuid,
            persistence,
        )
        .await?;
    }
    if persistence
        .repository_methods
        .iter()
        .any(|method| method == "enqueue_notification")
    {
        enqueue_outbox_events(
            pool,
            ctx,
            tenant_id,
            organization_id,
            persistence,
            input_snapshot,
        )
        .await?;
    }
    Ok(())
}

async fn mark_provider_submitted(
    pool: &DatabasePool,
    ctx: &PoolContext,
    tenant_id: i64,
    organization_id: i64,
    generation_uuid: &str,
    persistence: &ImageGenerationPersistencePlan,
    provider_state_json: serde_json::Value,
) -> Result<(), RepositoryError> {
    let table = ctx.table_name("image_generation_job");
    let poll_due = provider_poll_due(persistence.runtime_status);
    match pool {
        DatabasePool::Postgres(pg_pool, _) => {
            if poll_due {
                sqlx::query(&format!(
                    r#"
UPDATE {table}
SET provider_task_id = $5,
    provider_status = $6,
    provider_state = $7,
    job_status = $8,
    submitted_at = COALESCE(submitted_at, CURRENT_TIMESTAMP),
    next_poll_at = CURRENT_TIMESTAMP + INTERVAL '30 seconds',
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3 AND provider_code = $4
"#
                ))
                .bind(tenant_id)
                .bind(organization_id)
                .bind(generation_uuid)
                .bind(&persistence.provider_code)
                .bind(&persistence.provider_task_id)
                .bind(&persistence.provider_status)
                .bind(provider_state_json)
                .bind(persistence.job_status_code)
                .execute(pg_pool)
                .await?;
            } else {
                sqlx::query(&format!(
                    r#"
UPDATE {table}
SET provider_task_id = $5,
    provider_status = $6,
    provider_state = $7,
    job_status = $8,
    submitted_at = COALESCE(submitted_at, CURRENT_TIMESTAMP),
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3 AND provider_code = $4
"#
                ))
                .bind(tenant_id)
                .bind(organization_id)
                .bind(generation_uuid)
                .bind(&persistence.provider_code)
                .bind(&persistence.provider_task_id)
                .bind(&persistence.provider_status)
                .bind(provider_state_json)
                .bind(persistence.job_status_code)
                .execute(pg_pool)
                .await?;
            }
        }
    }
    Ok(())
}

async fn upsert_provider_task(
    pool: &DatabasePool,
    ctx: &PoolContext,
    tenant_id: i64,
    organization_id: i64,
    job_id: i64,
    persistence: &ImageGenerationPersistencePlan,
    input_snapshot: &ImageGenerationInputSnapshot,
    provider_request: &ImageProviderRequestSnapshot,
) -> Result<(), RepositoryError> {
    let Some(provider_task_id) = persistence
        .provider_task_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let task_uuid = uuid();
    let request_snapshot = serde_json::to_value(provider_request)?;
    let dispatch_status = dispatch_status_for_runtime(persistence.runtime_status);
    let poll_due = provider_poll_due(persistence.runtime_status);
    let table = ctx.table_name("image_provider_task");
    match pool {
        DatabasePool::Postgres(pg_pool, _) => {
            if poll_due {
                sqlx::query(&format!(
                    r#"
INSERT INTO {table} (
    uuid, tenant_id, organization_id, generation_job_id, provider_code,
    provider_operation, provider_task_id, provider_request_id, provider_status,
    dispatch_status, callback_url, request_snapshot, response_snapshot,
    next_poll_at, submitted_at, metadata
) VALUES (
    $1, $2, $3, $4, $5,
    $6, $7, $8, $9,
    $10, $11, $12, $13,
    CURRENT_TIMESTAMP + INTERVAL '30 seconds', CURRENT_TIMESTAMP, $14
)
ON CONFLICT (provider_code, provider_task_id)
WHERE provider_task_id IS NOT NULL
DO UPDATE SET
    provider_status = EXCLUDED.provider_status,
    dispatch_status = EXCLUDED.dispatch_status,
    response_snapshot = EXCLUDED.response_snapshot,
    next_poll_at = EXCLUDED.next_poll_at,
    updated_at = CURRENT_TIMESTAMP,
    version = image_provider_task.version + 1
"#
                ))
                .bind(&task_uuid)
                .bind(tenant_id)
                .bind(organization_id)
                .bind(job_id)
                .bind(&persistence.provider_code)
                .bind(&input_snapshot.provider_operation)
                .bind(provider_task_id)
                .bind::<Option<String>>(None)
                .bind(&persistence.provider_status)
                .bind(dispatch_status)
                .bind(&input_snapshot.callback_url)
                .bind(request_snapshot)
                .bind(serde_json::json!({}))
                .bind(serde_json::json!({ "generationId": persistence.generation_id }))
                .execute(pg_pool)
                .await?;
            } else {
                sqlx::query(&format!(
                    r#"
INSERT INTO {table} (
    uuid, tenant_id, organization_id, generation_job_id, provider_code,
    provider_operation, provider_task_id, provider_request_id, provider_status,
    dispatch_status, callback_url, request_snapshot, response_snapshot,
    submitted_at, completed_at, metadata
) VALUES (
    $1, $2, $3, $4, $5,
    $6, $7, $8, $9,
    $10, $11, $12, $13,
    CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, $14
)
ON CONFLICT (provider_code, provider_task_id)
WHERE provider_task_id IS NOT NULL
DO UPDATE SET
    provider_status = EXCLUDED.provider_status,
    dispatch_status = EXCLUDED.dispatch_status,
    response_snapshot = EXCLUDED.response_snapshot,
    completed_at = EXCLUDED.completed_at,
    updated_at = CURRENT_TIMESTAMP,
    version = image_provider_task.version + 1
"#
                ))
                .bind(&task_uuid)
                .bind(tenant_id)
                .bind(organization_id)
                .bind(job_id)
                .bind(&persistence.provider_code)
                .bind(&input_snapshot.provider_operation)
                .bind(provider_task_id)
                .bind::<Option<String>>(None)
                .bind(&persistence.provider_status)
                .bind(dispatch_status)
                .bind(&input_snapshot.callback_url)
                .bind(request_snapshot)
                .bind(serde_json::json!({}))
                .bind(serde_json::json!({ "generationId": persistence.generation_id }))
                .execute(pg_pool)
                .await?;
            }
        }
    }
    Ok(())
}

async fn mark_generation_failed(
    pool: &DatabasePool,
    ctx: &PoolContext,
    tenant_id: i64,
    organization_id: i64,
    generation_uuid: &str,
    persistence: &ImageGenerationPersistencePlan,
) -> Result<(), RepositoryError> {
    let table = ctx.table_name("image_generation_job");
    match pool {
        DatabasePool::Postgres(pg_pool, _) => {
            sqlx::query(&format!(
                r#"
UPDATE {table}
SET job_status = $4,
    provider_status = $5,
    finished_at = CURRENT_TIMESTAMP,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3
"#
            ))
            .bind(tenant_id)
            .bind(organization_id)
            .bind(generation_uuid)
            .bind(persistence.job_status_code)
            .bind(&persistence.provider_status)
            .execute(pg_pool)
            .await?;
        }
    }
    Ok(())
}

async fn enqueue_outbox_events(
    pool: &DatabasePool,
    ctx: &PoolContext,
    tenant_id: i64,
    organization_id: i64,
    persistence: &ImageGenerationPersistencePlan,
    input_snapshot: &ImageGenerationInputSnapshot,
) -> Result<(), RepositoryError> {
    if persistence.outbox_events.is_empty() {
        return Ok(());
    }
    let table = ctx.table_name("image_notification_outbox");
    for event in &persistence.outbox_events {
        enqueue_single_outbox_event(
            pool,
            &table,
            tenant_id,
            organization_id,
            event,
            persistence,
            input_snapshot,
        )
        .await?;
    }
    Ok(())
}

async fn enqueue_single_outbox_event(
    pool: &DatabasePool,
    table: &str,
    tenant_id: i64,
    organization_id: i64,
    event: &ImageGenerationOutboxEvent,
    persistence: &ImageGenerationPersistencePlan,
    input_snapshot: &ImageGenerationInputSnapshot,
) -> Result<(), RepositoryError> {
    let outbox_uuid = uuid();
    let payload = serde_json::json!({
        "eventType": event.event_type,
        "aggregateType": event.aggregate_type,
        "aggregateId": event.aggregate_id,
        "generationId": persistence.generation_id,
    });
    let metadata = serde_json::json!({
        "callbackUrl": input_snapshot.callback_url,
    });
    match pool {
        DatabasePool::Postgres(pg_pool, _) => {
            sqlx::query(&format!(
                r#"
INSERT INTO {table} (
    uuid, tenant_id, organization_id, aggregate_type, aggregate_id,
    event_type, payload_snapshot, delivery_status, next_delivery_at, metadata
) VALUES (
    $1, $2, $3, $4, $5,
    $6, $7, 'pending', CURRENT_TIMESTAMP, $8
)
"#
            ))
            .bind(&outbox_uuid)
            .bind(tenant_id)
            .bind(organization_id)
            .bind(&event.aggregate_type)
            .bind(&event.aggregate_id)
            .bind(&event.event_type)
            .bind(payload)
            .bind(metadata)
            .execute(pg_pool)
            .await?;
        }
    }
    Ok(())
}

fn provider_poll_due(status: ImageGenerationRuntimeStatus) -> bool {
    matches!(
        status,
        ImageGenerationRuntimeStatus::Queued
            | ImageGenerationRuntimeStatus::Dispatching
            | ImageGenerationRuntimeStatus::Submitted
            | ImageGenerationRuntimeStatus::Rendering
            | ImageGenerationRuntimeStatus::CancelRequested
    )
}

fn dispatch_status_for_runtime(status: ImageGenerationRuntimeStatus) -> &'static str {
    if provider_poll_due(status) {
        "rendering"
    } else if matches!(
        status,
        ImageGenerationRuntimeStatus::Failed
            | ImageGenerationRuntimeStatus::Cancelled
            | ImageGenerationRuntimeStatus::Expired
    ) {
        "failed"
    } else {
        "completed"
    }
}
