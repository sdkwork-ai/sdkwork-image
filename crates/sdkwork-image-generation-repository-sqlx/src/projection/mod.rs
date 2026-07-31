mod background;
mod lifecycle;
mod memory;
mod scope;
mod sqlx_store;

pub use background::{
    DueProviderTaskRow, ImageGenerationBackgroundRepository, PendingNotificationRow,
    SqlxImageGenerationBackgroundRepository,
};
pub use memory::InMemoryGenerationProjectionRepository;
pub use scope::{actor_user_id, parse_scope_id};
pub use sqlx_store::SqlxGenerationProjectionRepository;

use async_trait::async_trait;
use sdkwork_image_generation_service::ImageProviderDispatchPlan;
use sdkwork_image_generation_workflow_service::{
    rehydrate_image_provider_dispatch_plan, ImageGenerationPersistencePlan, ImageGenerationScope,
    ImageProviderRequestSnapshot,
};

use crate::RepositoryError;

#[derive(Clone, Debug)]
pub struct GenerationProjectionRecord {
    pub scope: ImageGenerationScope,
    pub generation_id: String,
    pub persistence: ImageGenerationPersistencePlan,
    pub provider_request: ImageProviderRequestSnapshot,
    pub wire_json: serde_json::Value,
}

impl GenerationProjectionRecord {
    pub fn dispatch_plan(&self) -> Result<ImageProviderDispatchPlan, RepositoryError> {
        rehydrate_image_provider_dispatch_plan(&self.provider_request)
            .map_err(|message| RepositoryError::Serialization(message.to_string()))
    }
}

#[async_trait]
pub trait GenerationProjectionRepository: Send + Sync {
    async fn insert(&self, record: GenerationProjectionRecord) -> Result<(), RepositoryError>;

    async fn get(
        &self,
        tenant_id: &str,
        generation_id: &str,
    ) -> Result<Option<GenerationProjectionRecord>, RepositoryError>;

    async fn list_wire_json(
        &self,
        tenant_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<serde_json::Value>, RepositoryError>;

    async fn update_after_refresh(
        &self,
        tenant_id: &str,
        organization_id: Option<&str>,
        generation_id: &str,
        wire_json: serde_json::Value,
        persistence: &ImageGenerationPersistencePlan,
    ) -> Result<(), RepositoryError>;

    async fn cancel_generation(
        &self,
        tenant_id: &str,
        organization_id: Option<&str>,
        generation_id: &str,
        wire_json: serde_json::Value,
    ) -> Result<(), RepositoryError>;
}
