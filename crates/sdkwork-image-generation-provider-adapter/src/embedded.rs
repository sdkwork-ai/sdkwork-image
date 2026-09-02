//! In-process CloudRouter dispatch adapter (APPLICATION_GATEWAY_SPEC §2.3).
//!
//! The HTTP adapter [`ImageGenerationProviderAdapter`] reaches CloudRouter's
//! open-api surface over an HTTP base URL, which is only correct when
//! CloudRouter runs as a separate process. When a host composes the
//! CloudRouter assembly in-process, pointing that base URL back at its own
//! listener would be the prohibited self-loop. This adapter keeps the exact
//! same open-api contract — the generated SDK's request/response models, the
//! `/v1`-prefixed open-api paths, and the API-key billing identity — but
//! dispatches through `tower::ServiceExt::oneshot` into the CloudRouter
//! assembly router. CloudRouter's surface dispatcher then forwards the request
//! into its edge-runtime gateway pipeline, so the full open-api
//! authentication/billing chain runs in-process, exactly as an external HTTP
//! request would.

use axum::body::Body;
use axum::extract::Request;
use cloudrouter_open_sdk::{
    MidjourneyImageGenerationTask, NanoBananaImageGenerationTask, OpenAiImageList,
    ViduImageGenerationTask, ViduTaskCreationsResponse,
};
use sdkwork_image_generation_provider_spi::{
    normalize_openai_image_generation_outputs, normalize_provider_task_generation_result,
    ImageGenerationCommand, ImageGenerationProvider, ImageGenerationProviderCapability,
    ImageGenerationProviderDescriptor, ImageGenerationProviderError, ImageGenerationProviderResult,
    ImageGenerationRuntimeStatus, ImageProviderDispatchPlan, ImageProviderOperation,
    ImageProviderSubmission, ImageVendorId, NormalizedProviderGenerationResult,
    OpenAiGeneratedImage, ProviderTaskSnapshot,
};

use crate::normalization::{
    normalize_vidu_image_generation_task_result, normalize_vidu_task_creations_result,
    provider_assets_from_generated_media, provider_error_from_sdk,
};
use crate::requests::{
    build_midjourney_image_generation_request, build_nano_banana_image_generation_request,
    build_openai_image_generation_request, build_vidu_reference_to_image_request,
};
use crate::routing::{provider_adapter_supports_retrieve_operation, resolve_sdk_operation_route};

use tower::ServiceExt;

/// Provider id for the in-process CloudRouter dispatch adapter. Distinct from
/// the HTTP adapter id so registries can tell which transport backs the
/// default provider.
pub const IMAGE_GENERATION_PROVIDER_CLOUDROUTER_EMBEDDED_ID: &str =
    "sdkwork-image-generation-provider-cloudrouter-embedded";

/// Mirrors the generated SDK's bounded-body budget
/// (`http::client::DEFAULT_MAX_RESPONSE_BODY_BYTES`).
const MAX_RESPONSE_BODY_BYTES: usize = 16 * 1024 * 1024;

// Open-api operation paths, mirroring the generated SDK's `ai_path` prefixing
// (`API_PREFIX = "/v1"`). Keep in sync with `resolve_sdk_operation_route`.
const OPENAI_IMAGES_GENERATIONS_PATH: &str = "/v1/images/generations";
const MIDJOURNEY_IMAGES_GENERATIONS_PATH: &str = "/v1/midjourney/v1/images/generations";
const NANO_BANANA_IMAGES_GENERATIONS_PATH: &str = "/v1/nano-banana/v1/images/generations";
const VIDU_REFERENCE_TO_IMAGE_PATH: &str = "/v1/vidu/ent/v2/reference2image";

/// In-process CloudRouter open-api dispatch adapter.
#[derive(Clone)]
pub struct CloudRouterEmbeddedImageGenerationAdapter {
    /// The CloudRouter API assembly contribution router (its surface
    /// dispatcher). Clone is cheap; `oneshot` consumes the clone per dispatch.
    router: axum::Router,
    api_key: Option<String>,
    descriptor: ImageGenerationProviderDescriptor,
}

impl CloudRouterEmbeddedImageGenerationAdapter {
    pub fn new(router: axum::Router, api_key: Option<String>) -> Self {
        Self {
            router,
            api_key,
            descriptor: ImageGenerationProviderDescriptor {
                id: IMAGE_GENERATION_PROVIDER_CLOUDROUTER_EMBEDDED_ID.to_string(),
                vendors: ["openai", "gpt-image", "midjourney", "nano-banana", "vidu"]
                    .into_iter()
                    .map(|vendor| ImageVendorId::new(vendor).expect("static vendor id"))
                    .collect(),
                capabilities: vec![
                    ImageGenerationProviderCapability::TextToImage,
                    ImageGenerationProviderCapability::ReferenceToImage,
                    ImageGenerationProviderCapability::MultipleOutputs,
                    ImageGenerationProviderCapability::NegativePrompt,
                    ImageGenerationProviderCapability::Seed,
                    ImageGenerationProviderCapability::Polling,
                    ImageGenerationProviderCapability::Webhook,
                ],
            },
        }
    }

    /// Reads the billing API key from the same environment the HTTP adapter
    /// uses, so deployments switch transports without re-configuring identity.
    pub fn from_env(router: axum::Router) -> Self {
        let api_key = std::env::var("SDKWORK_CLOUDROUTER_OPEN_API_KEY")
            .ok()
            .map(|key| key.trim().to_owned())
            .filter(|key| !key.is_empty());
        Self::new(router, api_key)
    }

    pub async fn create_async_image_generation(
        &self,
        plan: &ImageProviderDispatchPlan,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        match plan.provider_operation {
            ImageProviderOperation::OpenAiImageGeneration => {
                let request = build_openai_image_generation_request(plan)?;
                let response: OpenAiImageList = self
                    .dispatch_json(
                        axum::http::Method::POST,
                        OPENAI_IMAGES_GENERATIONS_PATH,
                        &serde_json::to_value(&request).map_err(serde_error)?,
                    )
                    .await?;
                let images = response
                    .data
                    .into_iter()
                    .map(|image| OpenAiGeneratedImage {
                        url: image.url,
                        b64_json: image.b64_json,
                        mime_type: image.mime_type,
                        revised_prompt: image.revised_prompt,
                    })
                    .collect::<Vec<_>>();
                let outputs =
                    normalize_openai_image_generation_outputs(&plan.provider_code, images)
                        .map_err(invalid_response)?;
                Ok(NormalizedProviderGenerationResult {
                    provider_code: plan.provider_code.clone(),
                    provider_task_id: None,
                    provider_status: Some("succeeded".to_string()),
                    provider_state: None,
                    status: ImageGenerationRuntimeStatus::Importing,
                    provider_terminal: true,
                    ready_for_drive_import: true,
                    outputs,
                    error_code: None,
                    error_message: None,
                })
            }
            ImageProviderOperation::MidjourneyImageGeneration => {
                let request = build_midjourney_image_generation_request(plan)?;
                let task: MidjourneyImageGenerationTask = self
                    .dispatch_json(
                        axum::http::Method::POST,
                        MIDJOURNEY_IMAGES_GENERATIONS_PATH,
                        &serde_json::to_value(&request).map_err(serde_error)?,
                    )
                    .await?;
                normalize_provider_task_generation_result(
                    &plan.provider_code,
                    ProviderTaskSnapshot {
                        task_id: task.task_id,
                        id: task.id,
                        status: task.status,
                        state: task.state,
                        model: task.model,
                        images: provider_assets_from_generated_media(
                            task.images.unwrap_or_default(),
                        ),
                        error: task.error.map(provider_error_from_sdk),
                    },
                )
                .map_err(invalid_response)
            }
            ImageProviderOperation::NanoBananaImageGeneration => {
                let request = build_nano_banana_image_generation_request(plan)?;
                let task: NanoBananaImageGenerationTask = self
                    .dispatch_json(
                        axum::http::Method::POST,
                        NANO_BANANA_IMAGES_GENERATIONS_PATH,
                        &serde_json::to_value(&request).map_err(serde_error)?,
                    )
                    .await?;
                normalize_provider_task_generation_result(
                    &plan.provider_code,
                    ProviderTaskSnapshot {
                        task_id: task.task_id,
                        id: task.id,
                        status: task.status,
                        state: task.state,
                        model: task.model,
                        images: provider_assets_from_generated_media(
                            task.images.unwrap_or_default(),
                        ),
                        error: task.error.map(provider_error_from_sdk),
                    },
                )
                .map_err(invalid_response)
            }
            ImageProviderOperation::ViduReferenceToImageGeneration => {
                let request = build_vidu_reference_to_image_request(plan)?;
                let task: ViduImageGenerationTask = self
                    .dispatch_json(
                        axum::http::Method::POST,
                        VIDU_REFERENCE_TO_IMAGE_PATH,
                        &serde_json::to_value(&request).map_err(serde_error)?,
                    )
                    .await?;
                normalize_vidu_image_generation_task_result(&plan.provider_code, task)
                    .map_err(invalid_response)
            }
            ImageProviderOperation::ProviderNativeImageGeneration => {
                Err(ImageGenerationProviderError::UnsupportedCapability(
                    "provider-native image generation".to_string(),
                ))
            }
        }
    }

    pub async fn retrieve_async_image_generation(
        &self,
        plan: &ImageProviderDispatchPlan,
        provider_task_id: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        match plan.provider_operation {
            ImageProviderOperation::MidjourneyImageGeneration => {
                let task: MidjourneyImageGenerationTask = self
                    .dispatch_json(
                        axum::http::Method::GET,
                        &format!("{MIDJOURNEY_IMAGES_GENERATIONS_PATH}/{provider_task_id}"),
                        &serde_json::Value::Null,
                    )
                    .await?;
                normalize_provider_task_generation_result(
                    &plan.provider_code,
                    ProviderTaskSnapshot {
                        task_id: task.task_id,
                        id: task.id,
                        status: task.status,
                        state: task.state,
                        model: task.model,
                        images: provider_assets_from_generated_media(
                            task.images.unwrap_or_default(),
                        ),
                        error: task.error.map(provider_error_from_sdk),
                    },
                )
                .map_err(invalid_response)
            }
            ImageProviderOperation::NanoBananaImageGeneration => {
                let task: NanoBananaImageGenerationTask = self
                    .dispatch_json(
                        axum::http::Method::GET,
                        &format!("{NANO_BANANA_IMAGES_GENERATIONS_PATH}/{provider_task_id}"),
                        &serde_json::Value::Null,
                    )
                    .await?;
                normalize_provider_task_generation_result(
                    &plan.provider_code,
                    ProviderTaskSnapshot {
                        task_id: task.task_id,
                        id: task.id,
                        status: task.status,
                        state: task.state,
                        model: task.model,
                        images: provider_assets_from_generated_media(
                            task.images.unwrap_or_default(),
                        ),
                        error: task.error.map(provider_error_from_sdk),
                    },
                )
                .map_err(invalid_response)
            }
            ImageProviderOperation::ViduReferenceToImageGeneration => {
                let task: ViduTaskCreationsResponse = self
                    .dispatch_json(
                        axum::http::Method::GET,
                        &format!("/v1/vidu/ent/v2/tasks/{provider_task_id}/creations"),
                        &serde_json::Value::Null,
                    )
                    .await?;
                normalize_vidu_task_creations_result(&plan.provider_code, task)
                    .map_err(invalid_response)
            }
            _ => Err(ImageGenerationProviderError::UnsupportedCapability(
                "task retrieval".to_string(),
            )),
        }
    }

    /// Dispatches one open-api operation into the embedded CloudRouter surface
    /// dispatcher and decodes the JSON response into the generated SDK model.
    async fn dispatch_json<T: serde::de::DeserializeOwned>(
        &self,
        method: axum::http::Method,
        path: &str,
        body: &serde_json::Value,
    ) -> ImageGenerationProviderResult<T> {
        let with_body = !body.is_null();
        let mut builder = Request::builder().method(method).uri(path);
        if let Some(api_key) = self.api_key.as_deref() {
            builder = builder.header("authorization", format!("Bearer {api_key}"));
        }
        if with_body {
            builder = builder.header("content-type", "application/json");
        }
        let payload = if with_body {
            serde_json::to_vec(body).map_err(serde_error)?
        } else {
            Vec::new()
        };
        let request = builder.body(Body::from(payload)).map_err(|error| {
            ImageGenerationProviderError::Configuration(format!(
                "embedded cloudrouter dispatch request build failed: {error}"
            ))
        })?;
        let response = match self.router.clone().oneshot(request).await {
            Ok(response) => response,
            Err(error) => match error {},
        };
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), MAX_RESPONSE_BODY_BYTES)
            .await
            .map_err(|error| {
                ImageGenerationProviderError::InvalidProviderResponse(format!(
                    "embedded cloudrouter response read failed: {error}"
                ))
            })?;
        if !status.is_success() {
            return Err(http_status_error(status.as_u16(), &body));
        }
        serde_json::from_slice(&body).map_err(|error| {
            ImageGenerationProviderError::InvalidProviderResponse(error.to_string())
        })
    }
}

fn http_status_error(status: u16, body: &[u8]) -> ImageGenerationProviderError {
    let body = String::from_utf8_lossy(body).to_string();
    match status {
        408 => ImageGenerationProviderError::Timeout(body),
        429 => ImageGenerationProviderError::RateLimited(body),
        status if status >= 500 => ImageGenerationProviderError::ProviderUnavailable(format!(
            "http status {status}: {body}"
        )),
        status => ImageGenerationProviderError::Rejected(format!("http status {status}: {body}")),
    }
}

fn invalid_response(message: &'static str) -> ImageGenerationProviderError {
    ImageGenerationProviderError::InvalidProviderResponse(message.to_string())
}

fn serde_error(error: serde_json::Error) -> ImageGenerationProviderError {
    ImageGenerationProviderError::InvalidRequest(error.to_string())
}

#[async_trait::async_trait]
impl ImageGenerationProvider for CloudRouterEmbeddedImageGenerationAdapter {
    fn descriptor(&self) -> &ImageGenerationProviderDescriptor {
        &self.descriptor
    }

    fn validate(&self, command: &ImageGenerationCommand) -> ImageGenerationProviderResult<()> {
        if !self.descriptor.supports_vendor(&command.vendor) {
            return Err(ImageGenerationProviderError::UnsupportedVendor(
                command.vendor.to_string(),
            ));
        }
        let plan =
            sdkwork_image_generation_provider_spi::plan_unified_image_generation_provider_dispatch(
                command,
            )
            .map_err(|message| ImageGenerationProviderError::InvalidRequest(message.to_string()))?;
        if resolve_sdk_operation_route(plan.provider_operation).is_none() {
            return Err(ImageGenerationProviderError::UnsupportedCapability(
                plan.provider_operation.as_str().to_string(),
            ));
        }
        Ok(())
    }

    async fn generate(
        &self,
        command: &ImageGenerationCommand,
    ) -> ImageGenerationProviderResult<ImageProviderSubmission> {
        self.validate(command)?;
        let mut dispatch_plan =
            sdkwork_image_generation_provider_spi::plan_unified_image_generation_provider_dispatch(
                command,
            )
            .map_err(|message| ImageGenerationProviderError::InvalidRequest(message.to_string()))?;
        dispatch_plan.provider_id = self.descriptor.id.clone();
        let result = self.create_async_image_generation(&dispatch_plan).await?;
        Ok(ImageProviderSubmission {
            dispatch_plan,
            result,
        })
    }

    async fn retrieve(
        &self,
        dispatch_plan: &ImageProviderDispatchPlan,
        provider_task_id: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        if !provider_adapter_supports_retrieve_operation(dispatch_plan) {
            return Err(ImageGenerationProviderError::UnsupportedCapability(
                "task retrieval".to_string(),
            ));
        }
        self.retrieve_async_image_generation(dispatch_plan, provider_task_id)
            .await
    }
}
