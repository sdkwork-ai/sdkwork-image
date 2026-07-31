//! Gateway bootstrap for sdkwork-image.
//! Generation app-api routes require an injected `ImageGenerationHost`.

use sdkwork_database_sqlx::DatabasePool;
use sdkwork_image_generation_host::ImageGenerationHost;
use sdkwork_web_bootstrap::{ApiAssemblyContribution, DatabasePoolReadinessCheck};
use std::sync::Arc;

pub struct ApiAssembly {
    pub contribution: ApiAssemblyContribution,
    pub background_processor: Option<tokio::task::JoinHandle<()>>,
}

pub async fn assemble_api_router_from_env() -> Result<ApiAssembly, String> {
    let host = ImageGenerationHost::from_runtime_env().await?;
    assemble_api_router(host)
}

pub async fn assemble_api_router_with_pool(pool: DatabasePool) -> Result<ApiAssembly, String> {
    let host = ImageGenerationHost::from_pool(pool).await?;
    assemble_api_router(host)
}

pub fn assemble_api_router(
    generation_host: Arc<ImageGenerationHost>,
) -> Result<ApiAssembly, String> {
    let database_pool = generation_host.database_pool().ok_or_else(|| {
        "image API assembly requires an ImageGenerationHost backed by a database pool".to_owned()
    })?;
    let router = sdkwork_routes_image_app_api::build_app_router(generation_host.clone());
    let contribution = ApiAssemblyContribution::from_manifest(
        "sdkwork-image",
        "SDKWork Image API",
        router,
        sdkwork_routes_image_app_api::gateway_route_manifest(),
        Vec::new(),
        Arc::new(DatabasePoolReadinessCheck::new(database_pool)),
    )?;
    let background_processor = generation_host.spawn_background_processor_if_enabled();
    Ok(ApiAssembly {
        contribution,
        background_processor,
    })
}
