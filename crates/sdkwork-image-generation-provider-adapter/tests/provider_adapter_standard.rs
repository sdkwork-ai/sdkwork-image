use sdkwork_image_generation_provider_adapter::{
    build_openai_image_generation_request, resolve_sdk_operation_route,
    IMAGE_GENERATION_PROVIDER_ADAPTER_ID,
};
use sdkwork_image_generation_provider_spi::{
    plan_unified_image_generation_provider_dispatch, ImageGenerationCommand,
    ImageGenerationGeometry, ImageGenerationModelSelection, ImageGenerationVendorParameters,
    ImageProviderOperation, ImageVendorId,
};

fn command(vendor: &str, model: &str) -> ImageGenerationCommand {
    ImageGenerationCommand {
        vendor: ImageVendorId::new(vendor).expect("vendor"),
        model: ImageGenerationModelSelection::named(model).expect("model"),
        prompt: "Product hero".to_string(),
        negative_prompt: Some("blur".to_string()),
        scene: "product_hero".to_string(),
        geometry: Some(ImageGenerationGeometry::Dimensions {
            width: 1536,
            height: 1024,
        }),
        quality: Some("high".to_string()),
        style: None,
        output_count: 2,
        reference_images: Vec::new(),
        callback_url: None,
        idempotency_key: Some("idem-001".to_string()),
        vendor_parameters: None,
    }
}

#[test]
fn sdk_routes_are_owned_only_by_the_adapter() {
    let openai = resolve_sdk_operation_route(ImageProviderOperation::OpenAiImageGeneration)
        .expect("openai route");

    assert_eq!(openai.create_resource, "images");
    assert_eq!(openai.create_method, "create_generation");
    assert!(resolve_sdk_operation_route(ImageProviderOperation::MidjourneyImageGeneration)
        .is_none());
    assert!(
        resolve_sdk_operation_route(ImageProviderOperation::NanoBananaImageGeneration).is_none()
    );
    assert_eq!(
        IMAGE_GENERATION_PROVIDER_ADAPTER_ID,
        "sdkwork-image-generation-provider-adapter"
    );
}

#[test]
fn maps_common_and_versioned_vendor_parameters_to_sdk_requests() {
    let mut openai = command("openai", "gpt-image-1");
    openai.vendor_parameters = Some(ImageGenerationVendorParameters {
        schema: "openai.image-generation.v1".to_string(),
        values: serde_json::json!({
            "quality": "medium",
            "response_format": "b64_json",
            "size": "1024x1024"
        }),
    });
    let plan = plan_unified_image_generation_provider_dispatch(&openai).expect("plan");
    let request = build_openai_image_generation_request(&plan).expect("request");
    assert_eq!(request.model, "gpt-image-1");
    assert_eq!(request.n, Some(2));
    assert_eq!(request.quality.as_deref(), Some("medium"));
    assert_eq!(request.response_format.as_deref(), Some("b64_json"));
}

#[test]
fn rejects_vendor_parameter_schema_mismatch() {
    let mut openai = command("openai", "gpt-image-1");
    openai.vendor_parameters = Some(ImageGenerationVendorParameters {
        schema: "vidu.image-generation.v1".to_string(),
        values: serde_json::json!({ "seed": 42 }),
    });
    let plan = plan_unified_image_generation_provider_dispatch(&openai).expect("plan");
    let error = build_openai_image_generation_request(&plan).expect_err("schema mismatch");
    assert!(error.to_string().contains("schema"));
}
