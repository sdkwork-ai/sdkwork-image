mod bootstrap;
mod catalog;
mod error;
mod projection;

pub use bootstrap::{
    bootstrap_image_database, bootstrap_image_database_from_env,
    connect_and_bootstrap_image_database_from_env, connect_image_database_pool_from_env,
    ImageDatabaseHost, ImageDatabasePool,
};
pub use catalog::{
    ImageAssetRecord, ImageCatalogRepository, ImageCatalogScope, ImageEditTaskCreateCommand,
    ImageEditTaskRecord, ImageGalleryItemCreateCommand, ImageGalleryItemRecord, ImageGalleryRecord,
    ImagePresetRecord, InMemoryImageCatalogRepository, SqlxImageCatalogRepository,
};
pub use error::RepositoryError;
pub use projection::{
    actor_user_id, parse_scope_id, DueProviderTaskRow, GenerationProjectionRecord,
    GenerationProjectionRepository, ImageGenerationBackgroundRepository,
    InMemoryGenerationProjectionRepository, PendingNotificationRow,
    SqlxGenerationProjectionRepository, SqlxImageGenerationBackgroundRepository,
};

pub const IMAGE_INITIAL_MIGRATION: &str = "0001_image_foundation.sql";
pub const IMAGE_RUNTIME_MIGRATION: &str = "0002_image_generation_drive_runtime.sql";

const IMAGE_INITIAL_MIGRATION_SQL: &str = include_str!("../migrations/0001_image_foundation.sql");
const IMAGE_RUNTIME_MIGRATION_SQL: &str =
    include_str!("../migrations/0002_image_generation_drive_runtime.sql");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageRepositoryBinding {
    pub domain: &'static str,
    pub repository_name: &'static str,
    pub tables: Vec<&'static str>,
    pub requires_transaction: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageStorageCapabilityManifest {
    pub name: &'static str,
    pub schema_version: &'static str,
    pub tables: Vec<&'static str>,
    pub generation_tables: Vec<&'static str>,
    pub asset_tables: Vec<&'static str>,
    pub gallery_tables: Vec<&'static str>,
    pub migrations: Vec<&'static str>,
    pub repository_bindings: Vec<ImageRepositoryBinding>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageRepositorySqlMethod {
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageGenerationRepositorySqlContract {
    pub repository_name: &'static str,
    pub requires_transaction: bool,
    pub methods: Vec<ImageRepositorySqlMethod>,
}

pub fn image_generation_tables() -> Vec<&'static str> {
    vec![
        "image_preset",
        "image_generation_job",
        "image_generation_output",
        "image_provider_binding",
        "image_provider_task",
        "image_provider_webhook_event",
        "image_notification_outbox",
        "image_edit_task",
    ]
}

pub fn image_asset_tables() -> Vec<&'static str> {
    vec!["image_asset"]
}

pub fn image_gallery_tables() -> Vec<&'static str> {
    vec!["image_gallery", "image_gallery_item"]
}

pub fn image_database_tables() -> Vec<&'static str> {
    let mut tables = image_generation_tables();
    tables.extend(image_asset_tables());
    tables.extend(image_gallery_tables());
    tables
}

pub fn image_initial_migration_sql() -> &'static str {
    IMAGE_INITIAL_MIGRATION_SQL
}

pub fn image_runtime_migration_sql() -> &'static str {
    IMAGE_RUNTIME_MIGRATION_SQL
}

pub fn image_storage_capability_manifest() -> ImageStorageCapabilityManifest {
    ImageStorageCapabilityManifest {
        name: "image-storage",
        schema_version: "2026-06-06",
        tables: image_database_tables(),
        generation_tables: image_generation_tables(),
        asset_tables: image_asset_tables(),
        gallery_tables: image_gallery_tables(),
        migrations: vec![IMAGE_INITIAL_MIGRATION, IMAGE_RUNTIME_MIGRATION],
        repository_bindings: vec![
            ImageRepositoryBinding {
                domain: "image",
                repository_name: "ImageGenerationRepository",
                tables: image_generation_tables(),
                requires_transaction: true,
            },
            ImageRepositoryBinding {
                domain: "image",
                repository_name: "ImageAssetRepository",
                tables: image_asset_tables(),
                requires_transaction: true,
            },
            ImageRepositoryBinding {
                domain: "image",
                repository_name: "ImageGalleryRepository",
                tables: image_gallery_tables(),
                requires_transaction: true,
            },
        ],
    }
}

pub fn image_generation_repository_sql_contract() -> ImageGenerationRepositorySqlContract {
    ImageGenerationRepositorySqlContract {
        repository_name: "ImageGenerationRepository",
        requires_transaction: true,
        methods: vec![
            ImageRepositorySqlMethod {
                name: "create_generation",
                sql: r#"
INSERT INTO image_generation_job (
    uuid, tenant_id, organization_id, user_id, request_id, trace_id,
    prompt, negative_prompt, resolution, style, model_id, provider_id,
    scene, provider_code, provider_operation, idempotency_key, callback_url,
    job_status, visibility, input_snapshot, drive_sync_status, queued_at, metadata
) VALUES (
    $1, $2, $3, $4, $5, $6,
    $7, $8, $9, $10, $11, $12,
    $13, $14, $15, $16, $17,
    $18, $19, $20, $21, CURRENT_TIMESTAMP, $22
)
RETURNING id, uuid
"#,
            },
            ImageRepositorySqlMethod {
                name: "mark_provider_submitted",
                sql: r#"
UPDATE image_generation_job
SET provider_task_id = $5,
    provider_status = $6,
    provider_state = $7,
    job_status = $8,
    submitted_at = CURRENT_TIMESTAMP,
    next_poll_at = $9,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $1
  AND organization_id = $2
  AND uuid = $3
  AND provider_code = $4
"#,
            },
            ImageRepositorySqlMethod {
                name: "upsert_provider_task",
                sql: r#"
INSERT INTO image_provider_task (
    uuid, tenant_id, organization_id, generation_job_id, provider_code,
    provider_operation, provider_task_id, provider_request_id, provider_status,
    dispatch_status, callback_url, request_snapshot, response_snapshot,
    next_poll_at, submitted_at, metadata
) VALUES (
    $1, $2, $3, $4, $5,
    $6, $7, $8, $9,
    $10, $11, $12, $13,
    $14, CURRENT_TIMESTAMP, $15
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
RETURNING id, uuid
"#,
            },
            ImageRepositorySqlMethod {
                name: "record_provider_webhook_event",
                sql: r#"
INSERT INTO image_provider_webhook_event (
    uuid, tenant_id, organization_id, provider_code, provider_task_id,
    external_event_id, event_type, payload_hash, signature_valid,
    payload_snapshot, process_status, metadata
) VALUES (
    $1, $2, $3, $4, $5,
    $6, $7, $8, $9,
    $10, $11, $12
)
ON CONFLICT (provider_code, payload_hash)
DO UPDATE SET
    process_status = image_provider_webhook_event.process_status,
    updated_at = CURRENT_TIMESTAMP
RETURNING id, uuid, process_status
"#,
            },
            ImageRepositorySqlMethod {
                name: "upsert_generation_outputs",
                sql: r#"
INSERT INTO image_generation_output (
    uuid, tenant_id, organization_id, user_id, generation_job_id, generation_uuid,
    output_index, media_kind, scene, provider_code, provider_operation, provider_task_id,
    provider_asset_id, provider_uri, provider_url, provider_payload_snapshot,
    drive_space_type, drive_space_id, drive_parent_node_id, drive_node_id, drive_uri,
    resource_snapshot, file_name, mime_type, size_bytes, width, height, duration_seconds,
    sync_status, metadata
) VALUES (
    $1, $2, $3, $4, $5, $6,
    $7, $8, $9, $10, $11, $12,
    $13, $14, $15, $16,
    $17, $18, $19, $20, $21,
    $22, $23, $24, $25, $26, $27, $28,
    $29, $30
)
ON CONFLICT (tenant_id, organization_id, generation_job_id, output_index)
DO UPDATE SET
    scene = EXCLUDED.scene,
    provider_code = EXCLUDED.provider_code,
    provider_operation = EXCLUDED.provider_operation,
    provider_task_id = EXCLUDED.provider_task_id,
    provider_asset_id = EXCLUDED.provider_asset_id,
    provider_uri = EXCLUDED.provider_uri,
    provider_url = EXCLUDED.provider_url,
    drive_space_id = EXCLUDED.drive_space_id,
    drive_parent_node_id = EXCLUDED.drive_parent_node_id,
    drive_node_id = EXCLUDED.drive_node_id,
    drive_uri = EXCLUDED.drive_uri,
    resource_snapshot = EXCLUDED.resource_snapshot,
    sync_status = EXCLUDED.sync_status,
    updated_at = CURRENT_TIMESTAMP,
    version = image_generation_output.version + 1
RETURNING id, uuid
"#,
            },
            ImageRepositorySqlMethod {
                name: "mark_drive_importing",
                sql: r#"
UPDATE image_generation_output
SET sync_status = 'importing',
    import_attempts = import_attempts + 1,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $1
  AND organization_id = $2
  AND generation_uuid = $3
  AND output_index = $4
"#,
            },
            ImageRepositorySqlMethod {
                name: "mark_drive_imported",
                sql: r#"
UPDATE image_generation_output
SET sync_status = 'imported',
    drive_space_id = $5,
    drive_node_id = $6,
    drive_uri = $7,
    object_blob_id = $8,
    resource_snapshot = $9,
    imported_at = CURRENT_TIMESTAMP,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $1
  AND organization_id = $2
  AND generation_uuid = $3
  AND output_index = $4
"#,
            },
            ImageRepositorySqlMethod {
                name: "mark_generation_succeeded",
                sql: r#"
UPDATE image_generation_job
SET job_status = $4,
    drive_sync_status = 'imported',
    output_asset_count = (
        SELECT COUNT(1)
        FROM image_generation_output
        WHERE tenant_id = $1
          AND organization_id = $2
          AND generation_uuid = $3
          AND sync_status = 'imported'
    ),
    finished_at = CURRENT_TIMESTAMP,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $1
  AND organization_id = $2
  AND uuid = $3
"#,
            },
            ImageRepositorySqlMethod {
                name: "mark_generation_failed",
                sql: r#"
UPDATE image_generation_job
SET job_status = $4,
    provider_status = $5,
    error_code = $6,
    error_message = $7,
    finished_at = CURRENT_TIMESTAMP,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE tenant_id = $1
  AND organization_id = $2
  AND uuid = $3
"#,
            },
            ImageRepositorySqlMethod {
                name: "enqueue_notification",
                sql: r#"
INSERT INTO image_notification_outbox (
    uuid, tenant_id, organization_id, aggregate_type, aggregate_id,
    event_type, payload_snapshot, delivery_status, next_delivery_at, metadata
) VALUES (
    $1, $2, $3, $4, $5,
    $6, $7, 'pending', $8, $9
)
RETURNING id, uuid
"#,
            },
            ImageRepositorySqlMethod {
                name: "find_due_provider_tasks",
                sql: r#"
SELECT *
FROM image_provider_task
WHERE tenant_id = $1
  AND organization_id = $2
  AND dispatch_status IN ('submitted', 'rendering', 'pending')
  AND (next_poll_at IS NULL OR next_poll_at <= CURRENT_TIMESTAMP)
ORDER BY next_poll_at NULLS FIRST, id
LIMIT $3
"#,
            },
            ImageRepositorySqlMethod {
                name: "find_pending_drive_imports",
                sql: r#"
SELECT *
FROM image_generation_output
WHERE tenant_id = $1
  AND organization_id = $2
  AND sync_status IN ('pending', 'failed')
  AND (next_retry_at IS NULL OR next_retry_at <= CURRENT_TIMESTAMP)
ORDER BY next_retry_at NULLS FIRST, id
LIMIT $3
"#,
            },
        ],
    }
}
