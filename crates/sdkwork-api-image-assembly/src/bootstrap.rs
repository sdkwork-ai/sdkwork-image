//! Gateway bootstrap for sdkwork-image.
//! Generation app-api routes require an injected `ImageGenerationHost`.

use sdkwork_database_sqlx::DatabasePool;
use sdkwork_image_generation_host::ImageGenerationHost;
use sdkwork_web_bootstrap::{ApiAssemblyContribution, DatabasePoolReadinessCheck, WebModule};
use std::sync::Arc;

pub struct ApiAssembly {
    pub contribution: ApiAssemblyContribution,
    pub database_pool: DatabasePool,
    pub background_processor: Option<tokio::task::JoinHandle<()>>,
}

pub async fn assemble_api_router_from_env() -> Result<ApiAssembly, String> {
    let host = ImageGenerationHost::from_runtime_env().await?;
    assemble_api_router(host)
}

pub async fn assemble_api_router_with_pool(pool: DatabasePool) -> Result<ApiAssembly, String> {
    assemble_api_router_with_pool_and_provider(pool, None).await
}

/// Same as [`assemble_api_router_with_pool`] but accepts a composition-root
/// injected image generation provider (APPLICATION_GATEWAY_SPEC §2.3): hosts
/// that embed CloudRouter in-process pass the CloudRouter dispatch adapter
/// instead of letting the host build an HTTP client from
/// `SDKWORK_CLOUDROUTER_OPEN_API_BASE_URL`. `None` keeps the env-driven HTTP
/// provider used by standalone deployments.
pub async fn assemble_api_router_with_pool_and_provider(
    pool: DatabasePool,
    provider: Option<Arc<dyn sdkwork_image_generation_provider_spi::ImageGenerationProvider>>,
) -> Result<ApiAssembly, String> {
    let host = ImageGenerationHost::from_pool_with_provider(pool, provider).await?;
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
        Arc::new(DatabasePoolReadinessCheck::new(database_pool.clone())),
    )?;
    let background_processor = generation_host.spawn_background_processor_if_enabled();
    Ok(ApiAssembly {
        contribution,
        database_pool,
        background_processor,
    })
}

/// Canonical Web Module definition for this application
/// (API_ASSEMBLY_SPEC §4.1.1): the complete HTTP surface — every route,
/// manifest, and OpenAPI document of this owner — as one installable module.
pub async fn web_module() -> Result<WebModule, String> {
    Ok(WebModule::from_contribution(
        assemble_api_router_from_env().await?.contribution,
    ))
}

/// Same as [`web_module`] but composed on a process-shared database pool
/// (platform gateways, API_ASSEMBLY_SPEC §4.1.1).
pub async fn web_module_with_pool(pool: DatabasePool) -> Result<WebModule, String> {
    Ok(WebModule::from_contribution(
        assemble_api_router_with_pool(pool).await?.contribution,
    ))
}

/// Same as [`web_module_with_pool`] but hands the detached background
/// processor back to the host (platform gateways, API_ASSEMBLY_SPEC §4.1.1).
///
/// The module owns the complete route definition; only the long-running
/// generation worker is transferred, because task shutdown is a host concern.
/// Hosts that drop the handle leave the worker detached.
pub async fn web_module_with_pool_retaining_background(
    pool: DatabasePool,
) -> Result<(WebModule, Option<tokio::task::JoinHandle<()>>), String> {
    web_module_with_pool_retaining_background_with_provider(pool, None).await
}

/// Same as [`web_module_with_pool_retaining_background`] with a
/// composition-root injected provider (APPLICATION_GATEWAY_SPEC §2.3): hosts
/// embedding CloudRouter in-process pass the CloudRouter dispatch adapter so
/// generation never issues an HTTP request back into the host's own listener.
pub async fn web_module_with_pool_retaining_background_with_provider(
    pool: DatabasePool,
    provider: Option<Arc<dyn sdkwork_image_generation_provider_spi::ImageGenerationProvider>>,
) -> Result<(WebModule, Option<tokio::task::JoinHandle<()>>), String> {
    let assembly = assemble_api_router_with_pool_and_provider(pool, provider).await?;
    Ok((
        WebModule::from_contribution(assembly.contribution),
        assembly.background_processor,
    ))
}
