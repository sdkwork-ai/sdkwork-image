//! Default external-provider adapter for image generation.
//!
//! The public surface is provider-oriented. The generated unified SDK remains an internal
//! infrastructure dependency and is never exposed through the provider SPI.

mod adapter;
mod normalization;
mod requests;
mod routing;

pub use adapter::ImageGenerationProviderAdapter;
pub use normalization::{
    normalize_vidu_image_generation_task_result, normalize_vidu_task_creations_result,
};
pub use requests::{
    build_midjourney_image_generation_request, build_nano_banana_image_generation_request,
    build_openai_image_generation_request, build_vidu_reference_to_image_request,
    openai_image_generation_sdk_supports_output_count,
};
pub use routing::{
    provider_adapter_supports_create_operation, provider_adapter_supports_retrieve_operation,
    resolve_sdk_operation_route, SdkOperationRoute, IMAGE_GENERATION_PROVIDER_ADAPTER_ID,
};

pub const IMAGE_GENERATION_SDK_CRATE: &str = "cloudrouter_open_sdk";
pub const IMAGE_GENERATION_SDK_METHOD: &str = "images.create_generation";
