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
    client: SdkworkAiClient,
    descriptor: ImageGenerationProviderDescriptor,
}

impl ImageGenerationProviderAdapter {
    pub fn new(client: SdkworkAiClient) -> Self {
        Self {
            client,
            descriptor: ImageGenerationProviderDescriptor {
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
            },
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
                    .client
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
                    .client
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
                    .client
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
                    .client
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
                    .client
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
                    .client
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
                    .client
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
