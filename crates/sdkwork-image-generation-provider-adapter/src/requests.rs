use cloudrouter_open_sdk::{
    MidjourneyImageGenerationRequest, NanoBananaImageGenerationRequest,
    OpenAiImageGenerationRequest, ViduReferenceToImageRequest,
};
use sdkwork_image_generation_provider_spi::{
    ImageGenerationProviderError, ImageGenerationProviderResult, ImageProviderDispatchPlan,
};

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenAiVendorParameters {
    quality: Option<String>,
    response_format: Option<String>,
    size: Option<String>,
}

#[derive(Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct SeededVendorParameters {
    seed: Option<i64>,
    style: Option<String>,
    payload: Option<String>,
}

pub fn build_openai_image_generation_request(
    plan: &ImageProviderDispatchPlan,
) -> ImageGenerationProviderResult<OpenAiImageGenerationRequest> {
    let parameters: OpenAiVendorParameters =
        decode_vendor_parameters(plan, "openai.image-generation.v1")?;
    Ok(OpenAiImageGenerationRequest {
        model: plan
            .model
            .clone()
            .unwrap_or_else(|| plan.provider_code.clone()),
        n: Some(plan.output_count.into()),
        prompt: plan.prompt.clone(),
        quality: parameters.quality.or_else(|| plan.quality.clone()),
        response_format: parameters
            .response_format
            .or_else(|| plan.response_format.clone()),
        size: parameters.size.or_else(|| plan.size.clone()),
    })
}

pub fn build_midjourney_image_generation_request(
    plan: &ImageProviderDispatchPlan,
) -> ImageGenerationProviderResult<MidjourneyImageGenerationRequest> {
    let parameters: SeededVendorParameters =
        decode_vendor_parameters(plan, "midjourney.image-generation.v1")?;
    reject_payload_for_non_vidu(&parameters)?;
    Ok(MidjourneyImageGenerationRequest {
        aspect_ratio: aspect_ratio_from_size(plan.size.as_deref()),
        callback_url: plan.callback_url.clone(),
        model: plan.model.clone(),
        prompt: prompt_with_optional_negative(plan),
        seed: parameters.seed,
        style: parameters.style.or_else(|| plan.quality.clone()),
    })
}

pub fn build_nano_banana_image_generation_request(
    plan: &ImageProviderDispatchPlan,
) -> ImageGenerationProviderResult<NanoBananaImageGenerationRequest> {
    let parameters: SeededVendorParameters =
        decode_vendor_parameters(plan, "nano-banana.image-generation.v1")?;
    if parameters.style.is_some() || parameters.payload.is_some() {
        return Err(ImageGenerationProviderError::UnsupportedParameter(
            "nano-banana only accepts seed in vendor parameters".to_string(),
        ));
    }
    Ok(NanoBananaImageGenerationRequest {
        aspect_ratio: aspect_ratio_from_size(plan.size.as_deref()),
        callback_url: plan.callback_url.clone(),
        images: (!plan.reference_images.is_empty()).then(|| plan.reference_images.clone()),
        model: plan.model.clone(),
        prompt: prompt_with_optional_negative(plan),
        seed: parameters.seed,
        size: plan.size.clone(),
    })
}

pub fn build_vidu_reference_to_image_request(
    plan: &ImageProviderDispatchPlan,
) -> ImageGenerationProviderResult<ViduReferenceToImageRequest> {
    let parameters: SeededVendorParameters =
        decode_vendor_parameters(plan, "vidu.image-generation.v1")?;
    Ok(ViduReferenceToImageRequest {
        aspect_ratio: aspect_ratio_from_size(plan.size.as_deref()),
        callback_url: plan.callback_url.clone(),
        images: plan.reference_images.clone(),
        model: plan
            .model
            .clone()
            .unwrap_or_else(|| plan.provider_code.clone()),
        payload: parameters.payload,
        prompt: prompt_with_optional_negative(plan),
        seed: parameters.seed,
        style: parameters.style.or_else(|| plan.quality.clone()),
    })
}

pub fn openai_image_generation_sdk_supports_output_count() -> bool {
    true
}

fn decode_vendor_parameters<T>(
    plan: &ImageProviderDispatchPlan,
    expected_schema: &str,
) -> ImageGenerationProviderResult<T>
where
    T: serde::de::DeserializeOwned + Default,
{
    let Some(parameters) = plan.vendor_parameters.as_ref() else {
        return Ok(T::default());
    };
    if parameters.schema.trim() != expected_schema {
        return Err(ImageGenerationProviderError::UnsupportedParameter(format!(
            "vendor parameter schema {} is not valid for {}",
            parameters.schema, plan.provider_code
        )));
    }
    serde_json::from_value(parameters.values.clone()).map_err(|error| {
        ImageGenerationProviderError::InvalidRequest(format!(
            "invalid {} vendor parameters: {error}",
            plan.provider_code
        ))
    })
}

fn reject_payload_for_non_vidu(
    parameters: &SeededVendorParameters,
) -> ImageGenerationProviderResult<()> {
    if parameters.payload.is_some() {
        return Err(ImageGenerationProviderError::UnsupportedParameter(
            "payload is only supported by the vidu image adapter".to_string(),
        ));
    }
    Ok(())
}

fn prompt_with_optional_negative(plan: &ImageProviderDispatchPlan) -> String {
    match plan.negative_prompt.as_deref() {
        Some(negative_prompt) if !negative_prompt.trim().is_empty() => format!(
            "{}\n\nNegative prompt: {}",
            plan.prompt,
            negative_prompt.trim()
        ),
        _ => plan.prompt.clone(),
    }
}

fn aspect_ratio_from_size(size: Option<&str>) -> Option<String> {
    let size = size?.trim();
    if let Some((width, height)) = size.split_once(':') {
        let width = width.trim().parse::<i64>().ok()?;
        let height = height.trim().parse::<i64>().ok()?;
        if width <= 0 || height <= 0 {
            return None;
        }
        let divisor = gcd(width, height);
        return Some(format!("{}:{}", width / divisor, height / divisor));
    }
    let (width, height) = size.split_once('x')?;
    let width = width.trim().parse::<i64>().ok()?;
    let height = height.trim().parse::<i64>().ok()?;
    if width <= 0 || height <= 0 {
        return None;
    }
    let divisor = gcd(width, height);
    Some(format!("{}:{}", width / divisor, height / divisor))
}

fn gcd(mut left: i64, mut right: i64) -> i64 {
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    left.abs().max(1)
}
