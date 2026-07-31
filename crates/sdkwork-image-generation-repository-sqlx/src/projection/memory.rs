use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;
use sdkwork_image_generation_workflow_service::ImageGenerationPersistencePlan;

use super::{GenerationProjectionRecord, GenerationProjectionRepository};
use crate::RepositoryError;

#[derive(Default)]
pub struct InMemoryGenerationProjectionRepository {
    records: RwLock<HashMap<(String, String), GenerationProjectionRecord>>,
}

impl InMemoryGenerationProjectionRepository {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl GenerationProjectionRepository for InMemoryGenerationProjectionRepository {
    async fn insert(&self, record: GenerationProjectionRecord) -> Result<(), RepositoryError> {
        let key = (record.scope.tenant_id.clone(), record.generation_id.clone());
        self.records.write().insert(key, record);
        Ok(())
    }

    async fn get(
        &self,
        tenant_id: &str,
        generation_id: &str,
    ) -> Result<Option<GenerationProjectionRecord>, RepositoryError> {
        Ok(self
            .records
            .read()
            .get(&(tenant_id.to_string(), generation_id.to_string()))
            .cloned())
    }

    async fn list_wire_json(
        &self,
        tenant_id: &str,
        _limit: i64,
        _offset: i64,
    ) -> Result<Vec<serde_json::Value>, RepositoryError> {
        Ok(self
            .records
            .read()
            .values()
            .filter(|record| record.scope.tenant_id == tenant_id)
            .map(|record| record.wire_json.clone())
            .collect())
    }

    async fn update_after_refresh(
        &self,
        tenant_id: &str,
        organization_id: Option<&str>,
        generation_id: &str,
        wire_json: serde_json::Value,
        persistence: &ImageGenerationPersistencePlan,
    ) -> Result<(), RepositoryError> {
        let key = (tenant_id.to_string(), generation_id.to_string());
        let mut records = self.records.write();
        let Some(record) = records.get_mut(&key) else {
            return Err(RepositoryError::NotFound);
        };
        record.wire_json = wire_json;
        record.persistence = persistence.clone();
        let _ = organization_id;
        Ok(())
    }

    async fn cancel_generation(
        &self,
        tenant_id: &str,
        organization_id: Option<&str>,
        generation_id: &str,
        wire_json: serde_json::Value,
    ) -> Result<(), RepositoryError> {
        let key = (tenant_id.to_string(), generation_id.to_string());
        let mut records = self.records.write();
        let Some(record) = records.get_mut(&key) else {
            return Err(RepositoryError::NotFound);
        };
        if matches!(
            record
                .wire_json
                .get("status")
                .and_then(|value| value.as_str()),
            Some("succeeded" | "failed" | "cancelled" | "expired")
        ) {
            return Err(RepositoryError::Conflict(
                "generation cannot be cancelled".to_string(),
            ));
        }
        record.wire_json = wire_json;
        let _ = organization_id;
        Ok(())
    }
}
