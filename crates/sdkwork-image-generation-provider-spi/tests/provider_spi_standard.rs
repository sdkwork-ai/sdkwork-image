use std::sync::Arc;

use sdkwork_image_generation_provider_spi::{
    ImageGenerationCommand, ImageGenerationProvider, ImageGenerationProviderDescriptor,
    ImageGenerationProviderRegistry, ImageGenerationProviderResult,
    ImageGenerationVendorParameters, ImageProviderDispatchPlan, ImageProviderSubmission,
    ImageVendorId, NormalizedProviderGenerationResult,
};

struct ContractProvider {
    descriptor: ImageGenerationProviderDescriptor,
}

#[async_trait::async_trait]
impl ImageGenerationProvider for ContractProvider {
    fn with_caller(
        &self,
        _caller: &sdkwork_utils_rust::caller_identity::CallerCredentials,
    ) -> Arc<dyn ImageGenerationProvider> {
        Arc::new(ContractProvider {
            descriptor: self.descriptor.clone(),
        })
    }

    fn descriptor(&self) -> &ImageGenerationProviderDescriptor {
        &self.descriptor
    }

    fn validate(&self, _command: &ImageGenerationCommand) -> ImageGenerationProviderResult<()> {
        Ok(())
    }

    async fn generate(
        &self,
        _command: &ImageGenerationCommand,
    ) -> ImageGenerationProviderResult<ImageProviderSubmission> {
        unreachable!("registry contract does not execute providers")
    }

    async fn retrieve(
        &self,
        _dispatch_plan: &ImageProviderDispatchPlan,
        _provider_task_id: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        unreachable!("registry contract does not execute providers")
    }
}

#[test]
fn registry_selects_an_injected_provider_by_vendor() {
    let provider = Arc::new(ContractProvider {
        descriptor: ImageGenerationProviderDescriptor {
            id: "test-provider".to_string(),
            vendors: vec![ImageVendorId::new("openai").expect("vendor")],
            capabilities: Vec::new(),
        },
    });
    let registry = ImageGenerationProviderRegistry::builder()
        .register(provider)
        .expect("register")
        .default_provider("test-provider")
        .build()
        .expect("registry");

    let selected = registry
        .select_for_vendor(&ImageVendorId::new("openai").expect("vendor"))
        .expect("provider");
    assert_eq!(selected.descriptor().id, "test-provider");
}

#[test]
fn vendor_parameters_are_versioned_and_transport_neutral() {
    let parameters = ImageGenerationVendorParameters {
        schema: "openai.image-generation.v1".to_string(),
        values: serde_json::json!({ "response_format": "b64_json" }),
    };

    assert_eq!(parameters.schema, "openai.image-generation.v1");
    assert_eq!(parameters.values["response_format"], "b64_json");
}
