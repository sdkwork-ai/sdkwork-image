use cloudrouter_open_sdk::SdkworkAiClient;
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
use crate::routing::{
    provider_adapter_supports_retrieve_operation, resolve_sdk_operation_route,
    IMAGE_GENERATION_PROVIDER_ADAPTER_ID,
};

#[derive(Clone)]
pub struct ImageGenerationProviderAdapter {
    client: ImageClientSource,
    descriptor: ImageGenerationProviderDescriptor,
}

/// The client source for one adapter instance.
///
/// `Fixed` keeps a caller-supplied client (tests and hosts that pin one).
/// `Reloading` re-resolves `cloudrouter` per call and rebuilds the client when
/// the registry target changes, so a standalone image host follows an online
/// failover without a restart (`DISCOVERY_SPEC.md` §12.1).
#[derive(Clone)]
enum ImageClientSource {
    Fixed(SdkworkAiClient),
    Reloading {
        base_url: String,
        api_key: Option<String>,
        cache: std::sync::Arc<std::sync::RwLock<CachedImageClient>>,
    },
}

#[derive(Clone)]
struct CachedImageClient {
    base_url: String,
    client: std::sync::Arc<SdkworkAiClient>,
}

impl ImageGenerationProviderAdapter {
    pub fn new(client: SdkworkAiClient) -> Self {
        Self {
            client: ImageClientSource::Fixed(client),
            descriptor: Self::image_provider_descriptor(),
        }
    }

    /// Build an adapter that resolves `cloudrouter` through the registry on
    /// every call and rebuilds its client when the target changes.
    pub fn from_settings(
        base_url: impl Into<String>,
        api_key: Option<String>,
    ) -> Result<Self, ImageGenerationProviderError> {
        let configured = base_url.into();
        let initial = Self::build_image_client(&configured, api_key.as_deref())?;
        Ok(Self {
            client: ImageClientSource::Reloading {
                base_url: configured,
                api_key,
                cache: std::sync::Arc::new(std::sync::RwLock::new(initial)),
            },
            descriptor: Self::image_provider_descriptor(),
        })
    }

    /// The base URL the adapter currently targets (registry or configured).
    ///
    /// Refreshes the cache first, so diagnostics observe an online failover
    /// without waiting for the next image call.
    pub fn base_url(&self) -> String {
        match &self.client {
            ImageClientSource::Fixed(_) => String::new(),
            ImageClientSource::Reloading { cache, .. } => {
                let _ = self.client();
                cache
                    .read()
                    .map(|cached| cached.base_url.clone())
                    .unwrap_or_default()
            }
        }
    }

    fn client(&self) -> Result<std::sync::Arc<SdkworkAiClient>, ImageGenerationProviderError> {
        match &self.client {
            ImageClientSource::Fixed(client) => Ok(std::sync::Arc::new(client.clone())),
            ImageClientSource::Reloading {
                base_url,
                api_key,
                cache,
            } => {
                let desired = sdkwork_utils_rust::service_base_url::registry_base_url_or(
                    "cloudrouter",
                    base_url,
                );
                {
                    let guard = cache
                        .read()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if guard.base_url == desired {
                        return Ok(std::sync::Arc::clone(&guard.client));
                    }
                }
                let fresh = Self::build_image_client(&desired, api_key.as_deref())?;
                let mut guard = cache
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                guard.base_url = fresh.base_url;
                guard.client = fresh.client;
                Ok(std::sync::Arc::clone(&guard.client))
            }
        }
    }

    fn build_image_client(
        configured_base_url: &str,
        api_key: Option<&str>,
    ) -> Result<CachedImageClient, ImageGenerationProviderError> {
        let base_url = sdkwork_utils_rust::service_base_url::registry_base_url_or(
            "cloudrouter",
            configured_base_url,
        );
        if base_url.trim().is_empty() {
            return Err(ImageGenerationProviderError::ProviderUnavailable(
                "image generation requires a cloudrouter base URL".to_owned(),
            ));
        }
        let client = SdkworkAiClient::new_with_base_url(base_url.clone()).map_err(|error| {
            ImageGenerationProviderError::ProviderUnavailable(format!(
                "image generation SDK client init failed: {error}"
            ))
        })?;
        if let Some(api_key) = api_key.filter(|value| !value.trim().is_empty()) {
            client.set_api_key(api_key);
        }
        Ok(CachedImageClient {
            base_url,
            client: std::sync::Arc::new(client),
        })
    }

    fn image_provider_descriptor() -> ImageGenerationProviderDescriptor {
        ImageGenerationProviderDescriptor {
            id: IMAGE_GENERATION_PROVIDER_ADAPTER_ID.to_string(),
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
        }
    }

    pub async fn create_async_image_generation(
        &self,
        plan: &ImageProviderDispatchPlan,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        match plan.provider_operation {
            ImageProviderOperation::OpenAiImageGeneration => {
                let request = build_openai_image_generation_request(plan)?;
                let response = self
                    .client()?
                    .images()
                    .create_generation(&request)
                    .await
                    .map_err(map_sdk_error)?;
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
                let task = self
                    .client()?
                    .images_midjourney()
                    .create_v1_images_generation(&request)
                    .await
                    .map_err(map_sdk_error)?;
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
                let task = self
                    .client()?
                    .images_nano_banana()
                    .create_generations(&request)
                    .await
                    .map_err(map_sdk_error)?;
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
                let task = self
                    .client()?
                    .images_vidu()
                    .create_ent_v2_reference2image(&request)
                    .await
                    .map_err(map_sdk_error)?;
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
                let task = self
                    .client()?
                    .images_midjourney()
                    .list_v1_images_generations(provider_task_id)
                    .await
                    .map_err(map_sdk_error)?;
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
                let task = self
                    .client()?
                    .images_nano_banana()
                    .retrieve_generations(provider_task_id)
                    .await
                    .map_err(map_sdk_error)?;
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
                let task = self
                    .client()?
                    .videos_vidu()
                    .list_ent_v2_tasks_creations(provider_task_id)
                    .await
                    .map_err(map_sdk_error)?;
                normalize_vidu_task_creations_result(&plan.provider_code, task)
                    .map_err(invalid_response)
            }
            _ => Err(ImageGenerationProviderError::UnsupportedCapability(
                "task retrieval".to_string(),
            )),
        }
    }
}

#[async_trait::async_trait]
impl ImageGenerationProvider for ImageGenerationProviderAdapter {
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

fn map_sdk_error(error: cloudrouter_open_sdk::SdkworkError) -> ImageGenerationProviderError {
    match error {
        cloudrouter_open_sdk::SdkworkError::Http(error) if error.is_timeout() => {
            ImageGenerationProviderError::Timeout(error.to_string())
        }
        cloudrouter_open_sdk::SdkworkError::Http(error) => {
            ImageGenerationProviderError::Transport(error.to_string())
        }
        cloudrouter_open_sdk::SdkworkError::HttpStatus { status: 408, body } => {
            ImageGenerationProviderError::Timeout(body)
        }
        cloudrouter_open_sdk::SdkworkError::HttpStatus { status: 429, body } => {
            ImageGenerationProviderError::RateLimited(body)
        }
        cloudrouter_open_sdk::SdkworkError::HttpStatus { status, body } if status >= 500 => {
            ImageGenerationProviderError::ProviderUnavailable(format!(
                "http status {status}: {body}"
            ))
        }
        cloudrouter_open_sdk::SdkworkError::HttpStatus { status, body } => {
            ImageGenerationProviderError::Rejected(format!("http status {status}: {body}"))
        }
        cloudrouter_open_sdk::SdkworkError::Serialization(error) => {
            ImageGenerationProviderError::InvalidProviderResponse(error.to_string())
        }
        cloudrouter_open_sdk::SdkworkError::ResponseBodyTooLarge { .. } => {
            ImageGenerationProviderError::InvalidProviderResponse(error.to_string())
        }
        cloudrouter_open_sdk::SdkworkError::ApiStatus { code, trace_id } => {
            ImageGenerationProviderError::Rejected(format!(
                "api status {code} (traceId={trace_id})"
            ))
        }
        cloudrouter_open_sdk::SdkworkError::MissingAccessToken => {
            ImageGenerationProviderError::Configuration(error.to_string())
        }
        error @ (cloudrouter_open_sdk::SdkworkError::InvalidHeaderName(_)
        | cloudrouter_open_sdk::SdkworkError::InvalidHeaderValue(_)
        | cloudrouter_open_sdk::SdkworkError::InvalidHttpMethod(_)) => {
            ImageGenerationProviderError::Configuration(error.to_string())
        }
    }
}

fn invalid_response(message: &'static str) -> ImageGenerationProviderError {
    ImageGenerationProviderError::InvalidProviderResponse(message.to_string())
}
