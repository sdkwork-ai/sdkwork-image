use std::path::PathBuf;
use std::sync::Arc;

use sdkwork_database_config::DatabaseConfig;
use sdkwork_database_lifecycle::{lifecycle_options_from_env, LifecycleOrchestrator};
use sdkwork_database_spi::{DatabaseAssetProvider, DatabaseManifest, DefaultDatabaseModule};
use sdkwork_database_sqlx::{create_pool_from_config, DatabasePool};

pub struct ImageDatabaseHost {
    pool: DatabasePool,
    module: Arc<DefaultDatabaseModule>,
}

impl ImageDatabaseHost {
    pub fn pool(&self) -> &DatabasePool {
        &self.pool
    }

    pub fn module(&self) -> Arc<DefaultDatabaseModule> {
        self.module.clone()
    }
}

pub async fn bootstrap_image_database(pool: DatabasePool) -> Result<ImageDatabaseHost, String> {
    if pool.as_postgres().is_none() {
        return Err(
            "image authoritative-server database requires PostgreSQL; SQLite is client-local only"
                .to_owned(),
        );
    }
    let app_root = resolve_app_root();
    let module = Arc::new(
        DefaultDatabaseModule::from_app_root(&app_root)
            .map_err(|error| format!("load image database module failed: {error}"))?,
    );
    let manifest = DatabaseManifest::from_file(module.manifest_path())
        .map_err(|error| format!("read image database manifest failed: {error}"))?;
    let options = lifecycle_options_from_env("IMAGE", &manifest);
    let orchestrator =
        LifecycleOrchestrator::new(pool.clone(), module.clone()).with_applied_by("sdkwork-image");

    orchestrator
        .init()
        .await
        .map_err(|error| format!("image database init failed: {error}"))?;

    if options.auto_migrate {
        orchestrator
            .migrate()
            .await
            .map_err(|error| format!("image database migrate failed: {error}"))?;
    }

    Ok(ImageDatabaseHost { pool, module })
}

pub async fn bootstrap_image_database_from_env() -> Result<ImageDatabaseHost, String> {
    let _ = dotenvy::dotenv();
    let config = DatabaseConfig::from_env("IMAGE")
        .map_err(|error| format!("read image database config failed: {error}"))?;
    let pool = create_pool_from_config(config)
        .await
        .map_err(|error| format!("create image database pool failed: {error}"))?;
    bootstrap_image_database(pool).await
}

fn resolve_app_root() -> PathBuf {
    std::env::var("SDKWORK_IMAGE_APP_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .canonicalize()
                .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."))
        })
}
