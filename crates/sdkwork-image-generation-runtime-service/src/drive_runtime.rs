use std::path::PathBuf;
use std::sync::Arc;

use sdkwork_assets_ingestion::DriveImportPlan;
use sdkwork_drive_storage_local::LocalDriveObjectStore;
use sdkwork_drive_workspace_service::bootstrap::{
    bootstrap_drive_database, connect_drive_database_pool_from_env,
};
use sdkwork_drive_workspace_service::infrastructure::sql::uploader_store::SqlUploaderStore;
use sdkwork_drive_workspace_service::uploader::DriveUploaderService;
use sdkwork_image_generation_service::ImageGenerationActor;

use crate::drive_import::{
    build_drive_import_execution_context, execute_drive_import_uploads,
    CompletedDriveImportArtifact, ImageDriveUploadPreparation,
};
use crate::provider_fetch_http::HttpProviderArtifactFetcher;
use crate::ImageRuntimeError;

pub struct ImageDriveImportRuntime {
    uploader: DriveUploaderService<SqlUploaderStore>,
    object_store: LocalDriveObjectStore,
    fetcher: HttpProviderArtifactFetcher,
}

impl ImageDriveImportRuntime {
    pub async fn try_from_env() -> Result<Option<Arc<Self>>, String> {
        if !drive_import_enabled_from_env() {
            return Ok(None);
        }
        let drive_pool = match connect_drive_database_pool_from_env().await {
            Ok(pool) => pool,
            Err(error) => {
                tracing::debug!("drive database unavailable for image import: {error}");
                return Ok(None);
            }
        };
        bootstrap_drive_database(drive_pool.clone())
            .await
            .map_err(|error| error.to_string())?;
        let drive_pool = drive_pool
            .as_postgres()
            .cloned()
            .ok_or_else(|| "image Drive import requires a PostgreSQL database pool".to_string())?;
        let object_store_root = object_store_root_from_env();
        std::fs::create_dir_all(&object_store_root).map_err(|error| error.to_string())?;
        let fetcher = HttpProviderArtifactFetcher::new()?;
        Ok(Some(Arc::new(Self {
            uploader: DriveUploaderService::new(SqlUploaderStore::new(drive_pool)),
            object_store: LocalDriveObjectStore::new(object_store_root),
            fetcher,
        })))
    }

    pub async fn execute_imports(
        &self,
        tenant_id: &str,
        organization_id: Option<&str>,
        actor: &ImageGenerationActor,
        operator_id: &str,
        now_epoch_ms: i64,
        import_plan: &DriveImportPlan,
        preparations: &[ImageDriveUploadPreparation],
    ) -> Result<Vec<CompletedDriveImportArtifact>, ImageRuntimeError> {
        if preparations.is_empty() {
            return Ok(Vec::new());
        }
        let execution = build_drive_import_execution_context(
            tenant_id.to_string(),
            organization_id.map(str::to_string),
            actor,
            operator_id.to_string(),
            now_epoch_ms,
        )?;
        execute_drive_import_uploads(
            &self.uploader,
            &self.object_store,
            &self.fetcher,
            import_plan,
            preparations,
            &execution,
        )
        .await
    }
}

fn drive_import_enabled_from_env() -> bool {
    matches!(
        std::env::var("IMAGE_DRIVE_IMPORT_ENABLED")
            .unwrap_or_else(|_| "true".to_owned())
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn object_store_root_from_env() -> PathBuf {
    std::env::var("IMAGE_DRIVE_OBJECT_STORE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".data/image-drive-objects"))
}
