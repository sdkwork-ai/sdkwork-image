use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_database_sqlx::DatabasePool;
use sdkwork_utils_rust::uuid;

use super::{
    ImageAssetRecord, ImageCatalogRepository, ImageCatalogScope, ImageEditTaskCreateCommand,
    ImageEditTaskRecord, ImageGalleryItemCreateCommand, ImageGalleryItemRecord, ImageGalleryRecord,
    ImagePresetRecord,
};
use crate::parse_scope_id;
use crate::RepositoryError;

#[derive(Clone)]
pub struct SqlxImageCatalogRepository {
    pool: DatabasePool,
}

impl SqlxImageCatalogRepository {
    pub fn new(pool: DatabasePool) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

#[async_trait]
impl ImageCatalogRepository for SqlxImageCatalogRepository {
    async fn list_presets(
        &self,
        scope: &ImageCatalogScope,
        limit: i64,
        offset: i64,
        q: Option<&str>,
    ) -> Result<Vec<ImagePresetRecord>, RepositoryError> {
        let (tenant_id, organization_id) = scope_ids(scope)?;
        let limit = limit.clamp(1, 200);
        let offset = offset.max(0);
        let search = q.map(str::trim).filter(|value| !value.is_empty());
        match &self.pool {
            DatabasePool::Sqlite(_, _) => {
                return Err(RepositoryError::Database(
                    "image generation repository requires PostgreSQL; SQLite engine is not supported"
                        .to_string(),
                ));
            }
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_preset");
                let mut sql = format!(
                    r#"
SELECT uuid, preset_key, title, description, default_resolution, default_style
FROM {table}
WHERE tenant_id = $1
  AND organization_id = $2
  AND deleted_at IS NULL
  AND status = 1
"#
                );
                if search.is_some() {
                    sql.push_str(" AND (title ILIKE $5 OR preset_key ILIKE $5)");
                }
                sql.push_str(" ORDER BY updated_at DESC, id DESC LIMIT $3 OFFSET $4");
                let rows = if let Some(search) = search {
                    let pattern = format!("%{search}%");
                    sqlx::query_as::<_, (String, String, String, Option<String>, String, String)>(
                        sqlx::AssertSqlSafe(sql.as_str()),
                    )
                    .bind(tenant_id)
                    .bind(organization_id)
                    .bind(limit)
                    .bind(offset)
                    .bind(pattern)
                    .fetch_all(pool)
                    .await?
                } else {
                    sqlx::query_as::<_, (String, String, String, Option<String>, String, String)>(
                        sqlx::AssertSqlSafe(sql.as_str()),
                    )
                    .bind(tenant_id)
                    .bind(organization_id)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?
                };
                Ok(rows
                    .into_iter()
                    .map(
                        |(
                            preset_id,
                            preset_key,
                            title,
                            description,
                            default_resolution,
                            default_style,
                        )| {
                            ImagePresetRecord {
                                preset_id,
                                preset_key,
                                title,
                                description,
                                default_resolution,
                                default_style,
                                item_count: 0,
                            }
                        },
                    )
                    .collect())
            }
        }
    }

    async fn get_preset(
        &self,
        scope: &ImageCatalogScope,
        preset_id: &str,
    ) -> Result<Option<ImagePresetRecord>, RepositoryError> {
        let (tenant_id, organization_id) = scope_ids(scope)?;
        let row = match &self.pool {
            DatabasePool::Sqlite(_, _) => {
                return Err(RepositoryError::Database(
                    "image generation repository requires PostgreSQL; SQLite engine is not supported"
                        .to_string(),
                ));
            }
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_preset");
                sqlx::query_as::<_, (String, String, String, Option<String>, String, String)>(
                    sqlx::AssertSqlSafe(format!(
                        r#"
SELECT uuid, preset_key, title, description, default_resolution, default_style
FROM {table}
WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3 AND deleted_at IS NULL
"#
                    )),
                )
                .bind(tenant_id)
                .bind(organization_id)
                .bind(preset_id.trim())
                .fetch_optional(pool)
                .await?
            }
        };
        Ok(row.map(
            |(preset_id, preset_key, title, description, default_resolution, default_style)| {
                ImagePresetRecord {
                    preset_id,
                    preset_key,
                    title,
                    description,
                    default_resolution,
                    default_style,
                    item_count: 0,
                }
            },
        ))
    }

    async fn list_assets(
        &self,
        scope: &ImageCatalogScope,
        limit: i64,
        offset: i64,
        q: Option<&str>,
    ) -> Result<Vec<ImageAssetRecord>, RepositoryError> {
        let (tenant_id, organization_id) = scope_ids(scope)?;
        let limit = limit.clamp(1, 200);
        let offset = offset.max(0);
        let search = q.map(str::trim).filter(|value| !value.is_empty());
        match &self.pool {
            DatabasePool::Sqlite(_, _) => {
                return Err(RepositoryError::Database(
                    "image generation repository requires PostgreSQL; SQLite engine is not supported"
                        .to_string(),
                ));
            }
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_asset");
                let rows = if let Some(search) = search {
                    let pattern = format!("%{search}%");
                    sqlx::query_as::<
                        _,
                        (
                            String,
                            Option<String>,
                            Option<String>,
                            Option<String>,
                            Option<String>,
                            String,
                        ),
                    >(sqlx::AssertSqlSafe(format!(
                        r#"
SELECT uuid, title, prompt, resolution, mime_type, provenance
FROM {table}
WHERE tenant_id = $1 AND organization_id = $2 AND deleted_at IS NULL AND status = 1
  AND (title ILIKE $5 OR prompt ILIKE $5)
ORDER BY updated_at DESC, id DESC
LIMIT $3 OFFSET $4
"#
                    )))
                    .bind(tenant_id)
                    .bind(organization_id)
                    .bind(limit)
                    .bind(offset)
                    .bind(pattern)
                    .fetch_all(pool)
                    .await?
                } else {
                    sqlx::query_as::<
                        _,
                        (
                            String,
                            Option<String>,
                            Option<String>,
                            Option<String>,
                            Option<String>,
                            String,
                        ),
                    >(sqlx::AssertSqlSafe(format!(
                        r#"
SELECT uuid, title, prompt, resolution, mime_type, provenance
FROM {table}
WHERE tenant_id = $1 AND organization_id = $2 AND deleted_at IS NULL AND status = 1
ORDER BY updated_at DESC, id DESC
LIMIT $3 OFFSET $4
"#
                    )))
                    .bind(tenant_id)
                    .bind(organization_id)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?
                };
                Ok(rows
                    .into_iter()
                    .map(
                        |(asset_id, title, prompt, resolution, mime_type, provenance)| {
                            ImageAssetRecord {
                                asset_id,
                                title,
                                prompt,
                                resolution,
                                mime_type,
                                provenance,
                            }
                        },
                    )
                    .collect())
            }
        }
    }

    async fn get_asset(
        &self,
        scope: &ImageCatalogScope,
        asset_id: &str,
    ) -> Result<Option<ImageAssetRecord>, RepositoryError> {
        let (tenant_id, organization_id) = scope_ids(scope)?;
        let row = match &self.pool {
            DatabasePool::Sqlite(_, _) => {
                return Err(RepositoryError::Database(
                    "image generation repository requires PostgreSQL; SQLite engine is not supported"
                        .to_string(),
                ));
            }
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_asset");
                sqlx::query_as::<
                    _,
                    (
                        String,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        Option<String>,
                        String,
                    ),
                >(sqlx::AssertSqlSafe(format!(
                    r#"
SELECT uuid, title, prompt, resolution, mime_type, provenance
FROM {table}
WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3 AND deleted_at IS NULL
"#
                )))
                .bind(tenant_id)
                .bind(organization_id)
                .bind(asset_id.trim())
                .fetch_optional(pool)
                .await?
            }
        };
        Ok(row.map(
            |(asset_id, title, prompt, resolution, mime_type, provenance)| ImageAssetRecord {
                asset_id,
                title,
                prompt,
                resolution,
                mime_type,
                provenance,
            },
        ))
    }

    async fn list_galleries(
        &self,
        scope: &ImageCatalogScope,
        limit: i64,
        offset: i64,
        q: Option<&str>,
    ) -> Result<Vec<ImageGalleryRecord>, RepositoryError> {
        let (tenant_id, organization_id) = scope_ids(scope)?;
        let limit = limit.clamp(1, 200);
        let offset = offset.max(0);
        let search = q.map(str::trim).filter(|value| !value.is_empty());
        match &self.pool {
            DatabasePool::Sqlite(_, _) => {
                return Err(RepositoryError::Database(
                    "image generation repository requires PostgreSQL; SQLite engine is not supported"
                        .to_string(),
                ));
            }
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_gallery");
                let item_table = ctx.table_name("image_gallery_item");
                let rows = if let Some(search) = search {
                    let pattern = format!("%{search}%");
                    sqlx::query_as::<_, (String, String, String, Option<String>, i64)>(sqlx::AssertSqlSafe(format!(
                        r#"
SELECT g.uuid, g.gallery_key, g.title, g.description,
       COALESCE((SELECT COUNT(1) FROM {item_table} gi WHERE gi.gallery_id = g.id AND gi.deleted_at IS NULL), 0)
FROM {table} g
WHERE g.tenant_id = $1 AND g.organization_id = $2 AND g.deleted_at IS NULL AND g.status = 1
  AND (g.title ILIKE $5 OR g.gallery_key ILIKE $5)
ORDER BY g.updated_at DESC, g.id DESC
LIMIT $3 OFFSET $4
"#
                    )))
                    .bind(tenant_id)
                    .bind(organization_id)
                    .bind(limit)
                    .bind(offset)
                    .bind(pattern)
                    .fetch_all(pool)
                    .await?
                } else {
                    sqlx::query_as::<_, (String, String, String, Option<String>, i64)>(sqlx::AssertSqlSafe(format!(
                        r#"
SELECT g.uuid, g.gallery_key, g.title, g.description,
       COALESCE((SELECT COUNT(1) FROM {item_table} gi WHERE gi.gallery_id = g.id AND gi.deleted_at IS NULL), 0)
FROM {table} g
WHERE g.tenant_id = $1 AND g.organization_id = $2 AND g.deleted_at IS NULL AND g.status = 1
ORDER BY g.updated_at DESC, g.id DESC
LIMIT $3 OFFSET $4
"#
                    )))
                    .bind(tenant_id)
                    .bind(organization_id)
                    .bind(limit)
                    .bind(offset)
                    .fetch_all(pool)
                    .await?
                };
                Ok(rows
                    .into_iter()
                    .map(
                        |(gallery_id, gallery_key, title, description, item_count)| {
                            ImageGalleryRecord {
                                gallery_id,
                                gallery_key,
                                title,
                                description,
                                item_count: item_count as i32,
                            }
                        },
                    )
                    .collect())
            }
        }
    }

    async fn get_gallery(
        &self,
        scope: &ImageCatalogScope,
        gallery_id: &str,
    ) -> Result<Option<ImageGalleryRecord>, RepositoryError> {
        let items = self.list_galleries(scope, 200, 0, None).await?;
        Ok(items
            .into_iter()
            .find(|gallery| gallery.gallery_id == gallery_id.trim()))
    }

    async fn create_gallery_item(
        &self,
        scope: &ImageCatalogScope,
        gallery_id: &str,
        command: ImageGalleryItemCreateCommand,
    ) -> Result<ImageGalleryItemRecord, RepositoryError> {
        let (tenant_id, organization_id) = scope_ids(scope)?;
        let gallery = self
            .get_gallery(scope, gallery_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let asset = self
            .get_asset(scope, command.asset_id.trim())
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let item_id = uuid();
        let sort_order = command.sort_order.unwrap_or(0);
        let caption = command.caption.clone();
        match &self.pool {
            DatabasePool::Sqlite(_, _) => {
                return Err(RepositoryError::Database(
                    "image generation repository requires PostgreSQL; SQLite engine is not supported"
                        .to_string(),
                ));
            }
            DatabasePool::Postgres(pool, ctx) => {
                let gallery_table = ctx.table_name("image_gallery");
                let asset_table = ctx.table_name("image_asset");
                let item_table = ctx.table_name("image_gallery_item");
                let gallery_pk: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                    "SELECT id FROM {gallery_table} WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3"
                )))
                .bind(tenant_id)
                .bind(organization_id)
                .bind(gallery_id.trim())
                .fetch_one(pool)
                .await?;
                let asset_pk: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                    "SELECT id FROM {asset_table} WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3"
                )))
                .bind(tenant_id)
                .bind(organization_id)
                .bind(asset.asset_id.as_str())
                .fetch_one(pool)
                .await?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    r#"
INSERT INTO {item_table} (
    uuid, tenant_id, organization_id, gallery_id, asset_id, sort_order, caption
) VALUES ($1, $2, $3, $4, $5, $6, $7)
"#
                )))
                .bind(&item_id)
                .bind(tenant_id)
                .bind(organization_id)
                .bind(gallery_pk)
                .bind(asset_pk)
                .bind(sort_order)
                .bind(&caption)
                .execute(pool)
                .await?;
            }
        }
        Ok(ImageGalleryItemRecord {
            item_id,
            gallery_id: gallery.gallery_id,
            asset_id: asset.asset_id,
            sort_order,
            caption,
        })
    }

    async fn create_edit_task(
        &self,
        scope: &ImageCatalogScope,
        command: ImageEditTaskCreateCommand,
    ) -> Result<ImageEditTaskRecord, RepositoryError> {
        if command.source_asset_id.trim().is_empty() {
            return Err(RepositoryError::Validation(
                "source_asset_id is required".to_string(),
            ));
        }
        if command.edit_type.trim().is_empty() {
            return Err(RepositoryError::Validation(
                "edit_type is required".to_string(),
            ));
        }
        if command.prompt.trim().is_empty() {
            return Err(RepositoryError::Validation(
                "prompt is required".to_string(),
            ));
        }
        let asset = self
            .get_asset(scope, command.source_asset_id.trim())
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let (tenant_id, organization_id) = scope_ids(scope)?;
        let user_id = parse_scope_id(&scope.user_id, "user_id").unwrap_or(0);
        let task_id = uuid();
        match &self.pool {
            DatabasePool::Sqlite(_, _) => {
                return Err(RepositoryError::Database(
                    "image generation repository requires PostgreSQL; SQLite engine is not supported"
                        .to_string(),
                ));
            }
            DatabasePool::Postgres(pool, ctx) => {
                let asset_table = ctx.table_name("image_asset");
                let task_table = ctx.table_name("image_edit_task");
                let source_asset_pk: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                    "SELECT id FROM {asset_table} WHERE tenant_id = $1 AND organization_id = $2 AND uuid = $3"
                )))
                .bind(tenant_id)
                .bind(organization_id)
                .bind(asset.asset_id.as_str())
                .fetch_one(pool)
                .await?;
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    r#"
INSERT INTO {task_table} (
    uuid, tenant_id, organization_id, user_id, source_asset_id, edit_type,
    prompt, negative_prompt, job_status
) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1)
"#
                )))
                .bind(&task_id)
                .bind(tenant_id)
                .bind(organization_id)
                .bind(user_id)
                .bind(source_asset_pk)
                .bind(command.edit_type.trim())
                .bind(command.prompt.trim())
                .bind(command.negative_prompt)
                .execute(pool)
                .await?;
            }
        }
        Ok(ImageEditTaskRecord {
            task_id,
            source_asset_id: asset.asset_id,
            edit_type: command.edit_type.trim().to_string(),
            prompt: command.prompt.trim().to_string(),
            status: "queued".to_string(),
        })
    }

    async fn get_edit_task(
        &self,
        scope: &ImageCatalogScope,
        task_id: &str,
    ) -> Result<Option<ImageEditTaskRecord>, RepositoryError> {
        let (tenant_id, organization_id) = scope_ids(scope)?;
        let row = match &self.pool {
            DatabasePool::Sqlite(_, _) => {
                return Err(RepositoryError::Database(
                    "image generation repository requires PostgreSQL; SQLite engine is not supported"
                        .to_string(),
                ));
            }
            DatabasePool::Postgres(pool, ctx) => {
                let task_table = ctx.table_name("image_edit_task");
                let asset_table = ctx.table_name("image_asset");
                sqlx::query_as::<_, (String, String, String, String, i32)>(sqlx::AssertSqlSafe(
                    format!(
                        r#"
SELECT t.uuid, a.uuid, t.edit_type, t.prompt, t.job_status
FROM {task_table} t
JOIN {asset_table} a ON a.id = t.source_asset_id
WHERE t.tenant_id = $1 AND t.organization_id = $2 AND t.uuid = $3 AND t.deleted_at IS NULL
"#
                    ),
                ))
                .bind(tenant_id)
                .bind(organization_id)
                .bind(task_id.trim())
                .fetch_optional(pool)
                .await?
            }
        };
        Ok(row.map(
            |(task_id, source_asset_id, edit_type, prompt, job_status)| ImageEditTaskRecord {
                task_id,
                source_asset_id,
                edit_type,
                prompt,
                status: map_edit_task_status(job_status),
            },
        ))
    }
}

fn scope_ids(scope: &ImageCatalogScope) -> Result<(i64, i64), RepositoryError> {
    let tenant_id = parse_scope_id(&scope.tenant_id, "tenant_id")?;
    let organization_id = match scope.organization_id.as_deref() {
        Some(value) if !value.trim().is_empty() => parse_scope_id(value, "organization_id")?,
        _ => 0,
    };
    Ok((tenant_id, organization_id))
}

fn map_edit_task_status(job_status: i32) -> String {
    match job_status {
        1 => "queued".to_string(),
        2 => "rendering".to_string(),
        3 => "ready".to_string(),
        _ => "failed".to_string(),
    }
}
