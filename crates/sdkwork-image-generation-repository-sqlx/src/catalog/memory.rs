use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::RwLock;

use super::{
    ImageAssetRecord, ImageCatalogRepository, ImageCatalogScope, ImageEditTaskCreateCommand,
    ImageEditTaskRecord, ImageGalleryItemCreateCommand, ImageGalleryItemRecord, ImageGalleryRecord,
    ImagePresetRecord,
};
use crate::RepositoryError;

#[derive(Default)]
pub struct InMemoryImageCatalogRepository {
    presets: RwLock<Vec<ImagePresetRecord>>,
    assets: RwLock<Vec<ImageAssetRecord>>,
    galleries: RwLock<Vec<ImageGalleryRecord>>,
    gallery_items: RwLock<Vec<ImageGalleryItemRecord>>,
    edit_tasks: RwLock<Vec<ImageEditTaskRecord>>,
}

impl InMemoryImageCatalogRepository {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl ImageCatalogRepository for InMemoryImageCatalogRepository {
    async fn list_presets(
        &self,
        _scope: &ImageCatalogScope,
        _limit: i64,
        _offset: i64,
        _q: Option<&str>,
    ) -> Result<Vec<ImagePresetRecord>, RepositoryError> {
        Ok(self.presets.read().clone())
    }

    async fn get_preset(
        &self,
        _scope: &ImageCatalogScope,
        preset_id: &str,
    ) -> Result<Option<ImagePresetRecord>, RepositoryError> {
        Ok(self
            .presets
            .read()
            .iter()
            .find(|preset| preset.preset_id == preset_id)
            .cloned())
    }

    async fn list_assets(
        &self,
        _scope: &ImageCatalogScope,
        _limit: i64,
        _offset: i64,
        _q: Option<&str>,
    ) -> Result<Vec<ImageAssetRecord>, RepositoryError> {
        Ok(self.assets.read().clone())
    }

    async fn get_asset(
        &self,
        _scope: &ImageCatalogScope,
        asset_id: &str,
    ) -> Result<Option<ImageAssetRecord>, RepositoryError> {
        Ok(self
            .assets
            .read()
            .iter()
            .find(|asset| asset.asset_id == asset_id)
            .cloned())
    }

    async fn list_galleries(
        &self,
        _scope: &ImageCatalogScope,
        _limit: i64,
        _offset: i64,
        _q: Option<&str>,
    ) -> Result<Vec<ImageGalleryRecord>, RepositoryError> {
        Ok(self.galleries.read().clone())
    }

    async fn get_gallery(
        &self,
        _scope: &ImageCatalogScope,
        gallery_id: &str,
    ) -> Result<Option<ImageGalleryRecord>, RepositoryError> {
        Ok(self
            .galleries
            .read()
            .iter()
            .find(|gallery| gallery.gallery_id == gallery_id)
            .cloned())
    }

    async fn create_gallery_item(
        &self,
        _scope: &ImageCatalogScope,
        gallery_id: &str,
        command: ImageGalleryItemCreateCommand,
    ) -> Result<ImageGalleryItemRecord, RepositoryError> {
        let item = ImageGalleryItemRecord {
            item_id: format!("item-{}-{}", gallery_id, command.asset_id),
            gallery_id: gallery_id.to_string(),
            asset_id: command.asset_id,
            sort_order: command.sort_order.unwrap_or(0),
            caption: command.caption,
        };
        self.gallery_items.write().push(item.clone());
        Ok(item)
    }

    async fn create_edit_task(
        &self,
        _scope: &ImageCatalogScope,
        command: ImageEditTaskCreateCommand,
    ) -> Result<ImageEditTaskRecord, RepositoryError> {
        let task = ImageEditTaskRecord {
            task_id: format!("edit-{}", command.source_asset_id),
            source_asset_id: command.source_asset_id,
            edit_type: command.edit_type,
            prompt: command.prompt,
            status: "queued".to_string(),
        };
        self.edit_tasks.write().push(task.clone());
        Ok(task)
    }

    async fn get_edit_task(
        &self,
        _scope: &ImageCatalogScope,
        task_id: &str,
    ) -> Result<Option<ImageEditTaskRecord>, RepositoryError> {
        Ok(self
            .edit_tasks
            .read()
            .iter()
            .find(|task| task.task_id == task_id)
            .cloned())
    }
}
