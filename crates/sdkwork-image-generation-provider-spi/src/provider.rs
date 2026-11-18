use std::sync::Arc;

use async_trait::async_trait;
use sdkwork_utils_rust::caller_identity::CallerCredentials;

use crate::{
    ImageGenerationCommand, ImageGenerationProviderResult, ImageProviderDispatchPlan,
    ImageVendorId, NormalizedProviderGenerationResult,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImageGenerationProviderCapability {
    TextToImage,
    ReferenceToImage,
    MultipleOutputs,
    NegativePrompt,
    Seed,
    Polling,
    Webhook,
    Cancellation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageGenerationProviderDescriptor {
    pub id: String,
    pub vendors: Vec<ImageVendorId>,
    pub capabilities: Vec<ImageGenerationProviderCapability>,
}

impl ImageGenerationProviderDescriptor {
    pub fn supports_vendor(&self, vendor: &ImageVendorId) -> bool {
        self.vendors.iter().any(|candidate| candidate == vendor)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageGenerationProviderHealth {
    pub available: bool,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageProviderSubmission {
    pub dispatch_plan: ImageProviderDispatchPlan,
    pub result: NormalizedProviderGenerationResult,
}

#[async_trait]
pub trait ImageGenerationProvider: Send + Sync {
    /// Returns a provider bound to the caller's verified dual credentials for
    /// one user-scoped dispatch (`APP_SDK_INTEGRATION_SPEC.md` §5.2.1).
    ///
    /// The bound provider keeps the same registry target and client pool; its
    /// clients are keyed by `(base URL, credential fingerprint)` so one caller's
    /// client can never be reused with another caller's tokens.
    fn with_caller(&self, caller: &CallerCredentials) -> Arc<dyn ImageGenerationProvider>;

    fn descriptor(&self) -> &ImageGenerationProviderDescriptor;

    fn validate(&self, command: &ImageGenerationCommand) -> ImageGenerationProviderResult<()>;

    async fn generate(
        &self,
        command: &ImageGenerationCommand,
    ) -> ImageGenerationProviderResult<ImageProviderSubmission>;

    async fn retrieve(
        &self,
        dispatch_plan: &ImageProviderDispatchPlan,
        provider_task_id: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult>;

    async fn cancel(
        &self,
        _dispatch_plan: &ImageProviderDispatchPlan,
        _provider_task_id: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        Err(crate::ImageGenerationProviderError::UnsupportedCapability(
            "cancellation".to_string(),
        ))
    }

    async fn health(&self) -> ImageGenerationProviderResult<ImageGenerationProviderHealth> {
        Ok(ImageGenerationProviderHealth {
            available: true,
            detail: None,
        })
    }
}
