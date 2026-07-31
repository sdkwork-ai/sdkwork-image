use std::sync::Arc;

use sdkwork_image_generation_repository_sqlx::{
    ImageAssetRecord, ImageCatalogRepository, ImageCatalogScope, ImageEditTaskCreateCommand,
    ImageEditTaskRecord, ImageGalleryItemCreateCommand, ImageGalleryItemRecord, ImageGalleryRecord,
    ImagePresetRecord, RepositoryError,
};

use crate::subject::RuntimeSubject;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ImageCatalogServiceError {
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("resource not found")]
    NotFound,
    #[error("persistence failed: {0}")]
    Persistence(String),
}

pub struct ImageCatalogService {
    store: Arc<dyn ImageCatalogRepository>,
}

impl ImageCatalogService {
    pub fn new(store: Arc<dyn ImageCatalogRepository>) -> Self {
        Self { store }
    }

    pub async fn list_presets(
        &self,
        subject: &RuntimeSubject,
        page: i64,
        page_size: i64,
        q: Option<String>,
    ) -> Result<Vec<ImagePresetRecord>, ImageCatalogServiceError> {
        let offset = (page.max(1) - 1) * page_size;
        self.store
            .list_presets(&catalog_scope(subject), page_size, offset, q.as_deref())
            .await
            .map_err(map_catalog_error)
    }

    pub async fn get_preset(
        &self,
        subject: &RuntimeSubject,
        preset_id: &str,
    ) -> Result<ImagePresetRecord, ImageCatalogServiceError> {
        self.store
            .get_preset(&catalog_scope(subject), preset_id.trim())
            .await
            .map_err(map_catalog_error)?
            .ok_or(ImageCatalogServiceError::NotFound)
    }

    pub async fn list_assets(
        &self,
        subject: &RuntimeSubject,
        page: i64,
        page_size: i64,
        q: Option<String>,
    ) -> Result<Vec<ImageAssetRecord>, ImageCatalogServiceError> {
        let offset = (page.max(1) - 1) * page_size;
        self.store
            .list_assets(&catalog_scope(subject), page_size, offset, q.as_deref())
            .await
            .map_err(map_catalog_error)
    }

    pub async fn get_asset(
        &self,
        subject: &RuntimeSubject,
        asset_id: &str,
    ) -> Result<ImageAssetRecord, ImageCatalogServiceError> {
        self.store
            .get_asset(&catalog_scope(subject), asset_id.trim())
            .await
            .map_err(map_catalog_error)?
            .ok_or(ImageCatalogServiceError::NotFound)
    }

    pub async fn list_galleries(
        &self,
        subject: &RuntimeSubject,
        page: i64,
        page_size: i64,
        q: Option<String>,
    ) -> Result<Vec<ImageGalleryRecord>, ImageCatalogServiceError> {
        let offset = (page.max(1) - 1) * page_size;
        self.store
            .list_galleries(&catalog_scope(subject), page_size, offset, q.as_deref())
            .await
            .map_err(map_catalog_error)
    }

    pub async fn get_gallery(
        &self,
        subject: &RuntimeSubject,
        gallery_id: &str,
    ) -> Result<ImageGalleryRecord, ImageCatalogServiceError> {
        self.store
            .get_gallery(&catalog_scope(subject), gallery_id.trim())
            .await
            .map_err(map_catalog_error)?
            .ok_or(ImageCatalogServiceError::NotFound)
    }

    pub async fn create_gallery_item(
        &self,
        subject: &RuntimeSubject,
        gallery_id: &str,
        command: ImageGalleryItemCreateCommand,
    ) -> Result<ImageGalleryItemRecord, ImageCatalogServiceError> {
        if command.asset_id.trim().is_empty() {
            return Err(ImageCatalogServiceError::Validation(
                "assetId is required".to_string(),
            ));
        }
        self.store
            .create_gallery_item(&catalog_scope(subject), gallery_id.trim(), command)
            .await
            .map_err(map_catalog_error)
    }

    pub async fn create_edit_task(
        &self,
        subject: &RuntimeSubject,
        command: ImageEditTaskCreateCommand,
    ) -> Result<ImageEditTaskRecord, ImageCatalogServiceError> {
        self.store
            .create_edit_task(&catalog_scope(subject), command)
            .await
            .map_err(map_catalog_error)
    }

    pub async fn get_edit_task(
        &self,
        subject: &RuntimeSubject,
        task_id: &str,
    ) -> Result<ImageEditTaskRecord, ImageCatalogServiceError> {
        self.store
            .get_edit_task(&catalog_scope(subject), task_id.trim())
            .await
            .map_err(map_catalog_error)?
            .ok_or(ImageCatalogServiceError::NotFound)
    }
}

fn catalog_scope(subject: &RuntimeSubject) -> ImageCatalogScope {
    ImageCatalogScope {
        tenant_id: subject.tenant_id.clone(),
        organization_id: subject.organization_id.clone(),
        user_id: subject.user_id.clone(),
    }
}

fn map_catalog_error(error: RepositoryError) -> ImageCatalogServiceError {
    match error {
        RepositoryError::NotFound => ImageCatalogServiceError::NotFound,
        RepositoryError::Validation(message) => ImageCatalogServiceError::Validation(message),
        RepositoryError::Conflict(message)
        | RepositoryError::Database(message)
        | RepositoryError::Serialization(message) => ImageCatalogServiceError::Persistence(message),
    }
}
