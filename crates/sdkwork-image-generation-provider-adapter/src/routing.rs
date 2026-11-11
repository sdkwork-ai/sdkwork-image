use sdkwork_image_generation_provider_spi::{ImageProviderDispatchPlan, ImageProviderOperation};

pub const IMAGE_GENERATION_PROVIDER_ADAPTER_ID: &str = "sdkwork-image-generation-provider-adapter";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SdkOperationRoute {
    pub create_resource: &'static str,
    pub create_method: &'static str,
    pub retrieve_resource: Option<&'static str>,
    pub retrieve_method: Option<&'static str>,
}

pub fn resolve_sdk_operation_route(operation: ImageProviderOperation) -> Option<SdkOperationRoute> {
    match operation {
        ImageProviderOperation::OpenAiImageGeneration => Some(SdkOperationRoute {
            create_resource: "images",
            create_method: "create_generation",
            retrieve_resource: None,
            retrieve_method: None,
        }),
        ImageProviderOperation::MidjourneyImageGeneration
        | ImageProviderOperation::NanoBananaImageGeneration => None,
        ImageProviderOperation::ViduReferenceToImageGeneration => Some(SdkOperationRoute {
            create_resource: "images_vidu",
            create_method: "create_ent_v2_reference2image",
            retrieve_resource: Some("videos_vidu"),
            retrieve_method: Some("list_ent_v2_tasks_creations"),
        }),
        ImageProviderOperation::ProviderNativeImageGeneration => None,
    }
}

pub fn provider_adapter_supports_create_operation(plan: &ImageProviderDispatchPlan) -> bool {
    resolve_sdk_operation_route(plan.provider_operation).is_some()
}

pub fn provider_adapter_supports_retrieve_operation(plan: &ImageProviderDispatchPlan) -> bool {
    resolve_sdk_operation_route(plan.provider_operation)
        .is_some_and(|route| route.retrieve_method.is_some())
}
