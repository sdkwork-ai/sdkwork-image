use std::sync::Arc;

use sdkwork_iam_web_adapter::{
    build_web_framework_builder, iam_web_request_context_resolver_from_database_pool_for_audiences,
    iam_web_request_context_resolver_from_env, IamAuditEmitter, IamSecurityEventEmitter,
};
use sdkwork_web_bootstrap::{infra_public_path_prefixes, ApiModuleRegistry};

const APPLICATION_ID: &str = "sdkwork-image";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sdkwork_web_bootstrap::init_tracing_from_env();
    let runtime = sdkwork_api_image_assembly::assemble_api_router_from_env().await?;
    let environment = std::env::var("SDKWORK_ENVIRONMENT")
        .or_else(|_| std::env::var("SDKWORK_IMAGE_ENVIRONMENT"))
        .unwrap_or_else(|_| "development".to_owned());
    let production = matches!(
        environment.trim().to_ascii_lowercase().as_str(),
        "prod" | "production"
    );
    let resolver = if production {
        iam_web_request_context_resolver_from_database_pool_for_audiences(
            runtime.database_pool.clone(),
            &[APPLICATION_ID],
        )
        .await?
    } else {
        iam_web_request_context_resolver_from_env().await
    };
    let assembly = runtime.contribution;
    let mut framework = build_web_framework_builder(
        resolver,
        assembly.route_manifest.clone(),
        infra_public_path_prefixes(),
    );
    if production {
        let postgres_pool = runtime
            .database_pool
            .as_postgres()
            .cloned()
            .ok_or("production Image gateway requires PostgreSQL")?;
        framework = framework
            .audit_emitter(Arc::new(IamAuditEmitter::new(
                postgres_pool.clone(),
                APPLICATION_ID,
                environment.clone(),
            )))
            .security_event_emitter(Arc::new(IamSecurityEventEmitter::new(
                postgres_pool,
                environment,
            )));
    }
    let mut module_registry = ApiModuleRegistry::new();
    module_registry.add_modules(vec![assembly]);
    let app = module_registry
        .try_compose("SDKWork Image API")?
        .into_hosted(framework)
        .router;
    let bind = std::env::var("SDKWORK_IMAGE_APPLICATION_PUBLIC_INGRESS_BIND")?;
    let address = bind.parse()?;

    let _background_processor = runtime.background_processor;
    sdkwork_web_bootstrap::serve(app, address).await?;
    Ok(())
}
