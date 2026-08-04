use std::sync::Arc;

use cloudrouter_open_sdk::{SdkworkAiClient, SdkworkConfig};
use sdkwork_database_sqlx::DatabasePool;
use sdkwork_image_generation_provider_adapter::{
    ImageGenerationProviderAdapter, IMAGE_GENERATION_PROVIDER_ADAPTER_ID,
};
use sdkwork_image_generation_repository_sqlx::{
    bootstrap_image_database, connect_image_database_pool_from_env, GenerationProjectionRepository,
    ImageGenerationBackgroundRepository, InMemoryGenerationProjectionRepository,
    InMemoryImageCatalogRepository, SqlxGenerationProjectionRepository, SqlxImageCatalogRepository,
    SqlxImageGenerationBackgroundRepository,
};
use sdkwork_image_generation_service::{
    ImageGenerationProviderRegistry, ImageGenerationService as ProviderGenerationService,
};

use crate::background::{
    image_background_processor_enabled_from_env, spawn_image_generation_background_processor,
};
use crate::catalog_service::ImageCatalogService;
use crate::service::ImageGenerationService;

pub struct ImageGenerationHost {
    service: Arc<ImageGenerationService>,
    catalog: Arc<ImageCatalogService>,
    provider_service: Arc<ProviderGenerationService>,
    background: Option<Arc<dyn ImageGenerationBackgroundRepository>>,
    database_pool: Option<DatabasePool>,
}

impl ImageGenerationHost {
    pub fn new(
        provider_service: Arc<ProviderGenerationService>,
        store: Arc<dyn GenerationProjectionRepository>,
        catalog_store: Arc<dyn sdkwork_image_generation_repository_sqlx::ImageCatalogRepository>,
        drive_import: Option<
            Arc<sdkwork_image_generation_runtime_service::ImageDriveImportRuntime>,
        >,
        background: Option<Arc<dyn ImageGenerationBackgroundRepository>>,
    ) -> Arc<Self> {
        Self::build(
            provider_service,
            store,
            catalog_store,
            drive_import,
            background,
            None,
        )
    }

    fn build(
        provider_service: Arc<ProviderGenerationService>,
        store: Arc<dyn GenerationProjectionRepository>,
        catalog_store: Arc<dyn sdkwork_image_generation_repository_sqlx::ImageCatalogRepository>,
        drive_import: Option<
            Arc<sdkwork_image_generation_runtime_service::ImageDriveImportRuntime>,
        >,
        background: Option<Arc<dyn ImageGenerationBackgroundRepository>>,
        database_pool: Option<DatabasePool>,
    ) -> Arc<Self> {
        Arc::new(Self {
            service: Arc::new(ImageGenerationService::new(
                provider_service.clone(),
                store,
                drive_import,
            )),
            catalog: Arc::new(ImageCatalogService::new(catalog_store)),
            provider_service,
            background,
            database_pool,
        })
    }

    pub fn from_service_with_store(
        provider_service: Arc<ProviderGenerationService>,
        store: Arc<dyn GenerationProjectionRepository>,
        catalog_store: Arc<dyn sdkwork_image_generation_repository_sqlx::ImageCatalogRepository>,
        drive_import: Option<
            Arc<sdkwork_image_generation_runtime_service::ImageDriveImportRuntime>,
        >,
        background: Option<Arc<dyn ImageGenerationBackgroundRepository>>,
    ) -> Arc<Self> {
        Self::new(
            provider_service,
            store,
            catalog_store,
            drive_import,
            background,
        )
    }

    pub fn for_test(provider_service: Arc<ProviderGenerationService>) -> Arc<Self> {
        Self::new(
            provider_service,
            InMemoryGenerationProjectionRepository::new(),
            InMemoryImageCatalogRepository::new(),
            None,
            None,
        )
    }

    pub async fn from_runtime_env() -> Result<Arc<Self>, String> {
        let pool = connect_image_database_pool_from_env()
            .await
            .map_err(|error| format!("create image database pool failed: {error}"))?;
        Self::from_pool(pool).await
    }

    pub async fn from_pool(pool: DatabasePool) -> Result<Arc<Self>, String> {
        let provider_service = provider_service_from_env()?;
        let database = bootstrap_image_database(pool).await?;
        let pool = database.pool().clone();
        let store = SqlxGenerationProjectionRepository::new(pool.clone());
        let catalog_store = SqlxImageCatalogRepository::new(pool.clone());
        let background: Arc<dyn ImageGenerationBackgroundRepository> =
            SqlxImageGenerationBackgroundRepository::new(pool.clone());
        let drive_import =
            sdkwork_image_generation_runtime_service::ImageDriveImportRuntime::try_from_env()
                .await?;
        Ok(Self::build(
            provider_service,
            store,
            catalog_store,
            drive_import,
            Some(background),
            Some(pool),
        ))
    }

    pub fn database_pool(&self) -> Option<DatabasePool> {
        self.database_pool.clone()
    }

    pub fn service(&self) -> Arc<ImageGenerationService> {
        self.service.clone()
    }

    pub fn catalog(&self) -> Arc<ImageCatalogService> {
        self.catalog.clone()
    }

    pub fn provider_service(&self) -> Arc<ProviderGenerationService> {
        self.provider_service.clone()
    }

    pub fn spawn_background_processor_if_enabled(&self) -> Option<tokio::task::JoinHandle<()>> {
        if !image_background_processor_enabled_from_env() {
            return None;
        }
        let background = self.background.clone()?;
        Some(spawn_image_generation_background_processor(
            self.service.clone(),
            background,
        ))
    }
}

fn provider_service_from_env() -> Result<Arc<ProviderGenerationService>, String> {
    let base_url = std::env::var("SDKWORK_CLOUDROUTER_OPEN_API_BASE_URL")
        .or_else(|_| std::env::var("CLOUDROUTER_OPEN_API_BASE_URL"))
        .map_err(|_| {
            "SDKWORK_CLOUDROUTER_OPEN_API_BASE_URL (or CLOUDROUTER_OPEN_API_BASE_URL) is required"
                .to_string()
        })?;
    let client = SdkworkAiClient::new(SdkworkConfig::new(base_url.trim()))
        .map_err(|error| format!("cloud router client init failed: {error}"))?;
    if let Ok(api_key) = std::env::var("SDKWORK_CLOUDROUTER_OPEN_API_KEY") {
        if !api_key.trim().is_empty() {
            client.set_api_key(api_key.trim());
        }
    }
    let provider = Arc::new(ImageGenerationProviderAdapter::new(client));
    let registry = ImageGenerationProviderRegistry::builder()
        .register(provider)
        .map_err(|error| format!("image provider registration failed: {error}"))?
        .default_provider(IMAGE_GENERATION_PROVIDER_ADAPTER_ID)
        .build()
        .map_err(|error| format!("image provider registry failed: {error}"))?;
    Ok(Arc::new(ProviderGenerationService::new(registry)))
}

impl std::fmt::Debug for ImageGenerationHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageGenerationHost")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_wraps_provider_and_application_service() {
        let client =
            SdkworkAiClient::new(SdkworkConfig::new("http://127.0.0.1:0")).expect("client");
        let provider = Arc::new(ImageGenerationProviderAdapter::new(client));
        let registry = ImageGenerationProviderRegistry::builder()
            .register(provider)
            .expect("provider")
            .default_provider(IMAGE_GENERATION_PROVIDER_ADAPTER_ID)
            .build()
            .expect("registry");
        let host =
            ImageGenerationHost::for_test(Arc::new(ProviderGenerationService::new(registry)));
        assert!(Arc::strong_count(&host.service()) >= 1);
        assert!(Arc::strong_count(&host.provider_service()) >= 1);
    }
}
