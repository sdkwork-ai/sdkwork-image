//! Application host for image generation HTTP handlers.

mod background;
mod catalog_service;
mod host;
mod service;
mod subject;
mod wire;

pub use background::{
    image_background_processor_enabled_from_env, spawn_image_generation_background_processor,
};

pub use catalog_service::{ImageCatalogService, ImageCatalogServiceError};
pub use host::ImageGenerationHost;
pub use sdkwork_image_generation_repository_sqlx::{
    ImageAssetRecord, ImageEditTaskCreateCommand, ImageEditTaskRecord,
    ImageGalleryItemCreateCommand, ImageGalleryItemRecord, ImageGalleryRecord, ImagePresetRecord,
};
pub use service::{ImageGenerationService, ImageGenerationServiceError};
pub use subject::{runtime_subject_from_iam, RuntimeSubject};
pub use wire::{
    ImageGenerationCancelCommandWire, ImageGenerationCommandWire, ImageGenerationOutputWire,
    ImageGenerationRefreshCommandWire, ImageGenerationWire, ImageOperationCommandWire,
};
