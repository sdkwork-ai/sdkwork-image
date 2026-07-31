use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_database_sqlx::DatabasePool;

use crate::RepositoryError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DueProviderTaskRow {
    pub task_uuid: String,
    pub tenant_id: i64,
    pub organization_id: i64,
    pub generation_uuid: String,
    pub user_id: i64,
    pub provider_code: String,
    pub provider_task_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingNotificationRow {
    pub outbox_uuid: String,
    pub tenant_id: i64,
    pub organization_id: i64,
    pub aggregate_type: String,
    pub aggregate_id: String,
    pub event_type: String,
    pub payload_snapshot: serde_json::Value,
    pub metadata: serde_json::Value,
    pub delivery_attempts: i32,
}

#[async_trait]
pub trait ImageGenerationBackgroundRepository: Send + Sync {
    async fn find_due_provider_tasks(
        &self,
        limit: i64,
    ) -> Result<Vec<DueProviderTaskRow>, RepositoryError>;

    async fn schedule_provider_task_poll(
        &self,
        task_uuid: &str,
        poll_interval_seconds: i64,
    ) -> Result<(), RepositoryError>;

    async fn find_pending_notifications(
        &self,
        limit: i64,
    ) -> Result<Vec<PendingNotificationRow>, RepositoryError>;

    async fn mark_notification_delivered(&self, outbox_uuid: &str) -> Result<(), RepositoryError>;

    async fn mark_notification_failed(
        &self,
        outbox_uuid: &str,
        error_message: &str,
        retry_after_seconds: i64,
    ) -> Result<(), RepositoryError>;
}

#[derive(Clone)]
pub struct SqlxImageGenerationBackgroundRepository {
    pool: DatabasePool,
}

impl SqlxImageGenerationBackgroundRepository {
    pub fn new(pool: DatabasePool) -> Arc<Self> {
        Arc::new(Self { pool })
    }
}

#[async_trait]
impl ImageGenerationBackgroundRepository for SqlxImageGenerationBackgroundRepository {
    async fn find_due_provider_tasks(
        &self,
        limit: i64,
    ) -> Result<Vec<DueProviderTaskRow>, RepositoryError> {
        let limit = limit.clamp(1, 100);
        match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                let task_table = ctx.table_name("image_provider_task");
                let job_table = ctx.table_name("image_generation_job");
                let rows = sqlx::query_as::<_, (String, i64, i64, String, i64, String, String)>(&format!(
                    r#"
SELECT t.uuid, t.tenant_id, t.organization_id, j.uuid, COALESCE(j.user_id, 0), t.provider_code, t.provider_task_id
FROM {task_table} t
INNER JOIN {job_table} j ON j.id = t.generation_job_id
WHERE t.dispatch_status IN ('submitted', 'rendering', 'pending')
  AND t.provider_task_id IS NOT NULL
  AND (t.next_poll_at IS NULL OR t.next_poll_at <= CURRENT_TIMESTAMP)
  AND t.deleted_at IS NULL
  AND j.deleted_at IS NULL
ORDER BY t.next_poll_at NULLS FIRST, t.id
LIMIT $1
"#
                ))
                .bind(limit)
                .fetch_all(pool)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(
                        |(
                            task_uuid,
                            tenant_id,
                            organization_id,
                            generation_uuid,
                            user_id,
                            provider_code,
                            provider_task_id,
                        )| DueProviderTaskRow {
                            task_uuid,
                            tenant_id,
                            organization_id,
                            generation_uuid,
                            user_id,
                            provider_code,
                            provider_task_id,
                        },
                    )
                    .collect())
            }
        }
    }

    async fn schedule_provider_task_poll(
        &self,
        task_uuid: &str,
        poll_interval_seconds: i64,
    ) -> Result<(), RepositoryError> {
        let poll_interval_seconds = poll_interval_seconds.clamp(5, 3600);
        match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_provider_task");
                sqlx::query(&format!(
                    r#"
UPDATE {table}
SET poll_attempts = poll_attempts + 1,
    last_polled_at = CURRENT_TIMESTAMP,
    next_poll_at = CURRENT_TIMESTAMP + ($2 * INTERVAL '1 second'),
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE uuid = $1
"#
                ))
                .bind(task_uuid)
                .bind(poll_interval_seconds)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    async fn find_pending_notifications(
        &self,
        limit: i64,
    ) -> Result<Vec<PendingNotificationRow>, RepositoryError> {
        let limit = limit.clamp(1, 100);
        match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_notification_outbox");
                let rows = sqlx::query_as::<
                    _,
                    (
                        String,
                        i64,
                        i64,
                        String,
                        String,
                        String,
                        serde_json::Value,
                        serde_json::Value,
                        i32,
                    ),
                >(&format!(
                    r#"
SELECT uuid, tenant_id, organization_id, aggregate_type, aggregate_id, event_type,
       payload_snapshot, metadata, delivery_attempts
FROM {table}
WHERE delivery_status = 'pending'
  AND (next_delivery_at IS NULL OR next_delivery_at <= CURRENT_TIMESTAMP)
  AND deleted_at IS NULL
ORDER BY next_delivery_at NULLS FIRST, id
LIMIT $1
"#
                ))
                .bind(limit)
                .fetch_all(pool)
                .await?;
                Ok(rows
                    .into_iter()
                    .map(
                        |(
                            outbox_uuid,
                            tenant_id,
                            organization_id,
                            aggregate_type,
                            aggregate_id,
                            event_type,
                            payload_snapshot,
                            metadata,
                            delivery_attempts,
                        )| PendingNotificationRow {
                            outbox_uuid,
                            tenant_id,
                            organization_id,
                            aggregate_type,
                            aggregate_id,
                            event_type,
                            payload_snapshot,
                            metadata,
                            delivery_attempts,
                        },
                    )
                    .collect())
            }
        }
    }

    async fn mark_notification_delivered(&self, outbox_uuid: &str) -> Result<(), RepositoryError> {
        match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_notification_outbox");
                sqlx::query(&format!(
                    r#"
UPDATE {table}
SET delivery_status = 'delivered',
    delivered_at = CURRENT_TIMESTAMP,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE uuid = $1
"#
                ))
                .bind(outbox_uuid)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    async fn mark_notification_failed(
        &self,
        outbox_uuid: &str,
        error_message: &str,
        retry_after_seconds: i64,
    ) -> Result<(), RepositoryError> {
        let retry_after_seconds = retry_after_seconds.clamp(5, 86_400);
        match &self.pool {
            DatabasePool::Postgres(pool, ctx) => {
                let table = ctx.table_name("image_notification_outbox");
                sqlx::query(&format!(
                    r#"
UPDATE {table}
SET delivery_status = 'pending',
    delivery_attempts = delivery_attempts + 1,
    next_delivery_at = CURRENT_TIMESTAMP + ($3 * INTERVAL '1 second'),
    error_message = $2,
    updated_at = CURRENT_TIMESTAMP,
    version = version + 1
WHERE uuid = $1
"#
                ))
                .bind(outbox_uuid)
                .bind(error_message)
                .bind(retry_after_seconds)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }
}
