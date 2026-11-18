use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_image_generation_provider_spi::{
    ImageGenerationCommand, ImageGenerationProviderDescriptor, ImageGenerationProviderError,
    ImageGenerationProviderRegistry, ImageGenerationProviderResult, ImageProviderDispatchPlan,
    ImageProviderSubmission, ImageVendorId, NormalizedProviderGenerationResult,
};
use sdkwork_utils_rust::caller_identity::CallerCredentials;

#[async_trait]
pub trait ImageGenerationServicePort: Send + Sync {
    async fn generate(
        &self,
        command: ImageGenerationCommand,
    ) -> ImageGenerationProviderResult<ImageProviderSubmission>;

    async fn retrieve(
        &self,
        dispatch_plan: &ImageProviderDispatchPlan,
        provider_task_id: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult>;

    async fn cancel(
        &self,
        dispatch_plan: &ImageProviderDispatchPlan,
        provider_task_id: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult>;

    fn provider_descriptors(&self) -> Vec<ImageGenerationProviderDescriptor>;
}

#[derive(Clone)]
pub struct ImageGenerationService {
    providers: Arc<ImageGenerationProviderRegistry>,
    /// The caller whose dual tokens a user-scoped dispatch must carry. Empty on
    /// a background/worker path that uses the declared service identity.
    caller: CallerCredentials,
}

impl ImageGenerationService {
    pub fn new(providers: ImageGenerationProviderRegistry) -> Self {
        Self {
            providers: Arc::new(providers),
            caller: CallerCredentials::default(),
        }
    }

    /// Binds this service to the verified caller of one request
    /// (`APP_SDK_INTEGRATION_SPEC.md` §5.2.1). Each dispatch then selects a
    /// caller-bound provider; a caller-less service keeps the declared service
    /// identity for background work.
    #[must_use]
    pub fn with_caller(&self, caller: &CallerCredentials) -> Self {
        Self {
            providers: Arc::clone(&self.providers),
            caller: caller.clone(),
        }
    }

    fn provider_for_dispatch(
        &self,
        dispatch_plan: &ImageProviderDispatchPlan,
    ) -> ImageGenerationProviderResult<
        Arc<dyn sdkwork_image_generation_provider_spi::ImageGenerationProvider>,
    > {
        if !dispatch_plan.provider_id.trim().is_empty() {
            return self
                .providers
                .select_by_id(&dispatch_plan.provider_id)
                .map(|provider| provider.with_caller(&self.caller));
        }
        let vendor = ImageVendorId::new(&dispatch_plan.provider_code)?;
        self.providers
            .select_for_vendor(&vendor)
            .map(|provider| provider.with_caller(&self.caller))
    }
}

#[async_trait]
impl ImageGenerationServicePort for ImageGenerationService {
    async fn generate(
        &self,
        command: ImageGenerationCommand,
    ) -> ImageGenerationProviderResult<ImageProviderSubmission> {
        let provider = self.providers.select_for_vendor(&command.vendor)?;
        provider.validate(&command)?;
        provider.generate(&command).await
    }

    async fn retrieve(
        &self,
        dispatch_plan: &ImageProviderDispatchPlan,
        provider_task_id: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        if provider_task_id.trim().is_empty() {
            return Err(ImageGenerationProviderError::InvalidRequest(
                "provider_task_id is required".to_string(),
            ));
        }
        self.provider_for_dispatch(dispatch_plan)?
            .retrieve(dispatch_plan, provider_task_id.trim())
            .await
    }

    async fn cancel(
        &self,
        dispatch_plan: &ImageProviderDispatchPlan,
        provider_task_id: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        if provider_task_id.trim().is_empty() {
            return Err(ImageGenerationProviderError::InvalidRequest(
                "provider_task_id is required".to_string(),
            ));
        }
        self.provider_for_dispatch(dispatch_plan)?
            .cancel(dispatch_plan, provider_task_id.trim())
            .await
    }

    fn provider_descriptors(&self) -> Vec<ImageGenerationProviderDescriptor> {
        self.providers.descriptors()
    }
}
