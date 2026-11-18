use std::sync::Arc;

use sdkwork_image_generation_service::{
    plan_unified_image_generation_provider_dispatch, ImageGenerationCommand,
    ImageGenerationGeometry, ImageGenerationModelSelection, ImageGenerationProvider,
    ImageGenerationProviderDescriptor, ImageGenerationProviderRegistry,
    ImageGenerationProviderResult, ImageGenerationRuntimeStatus, ImageGenerationService,
    ImageGenerationServicePort, ImageProviderDispatchPlan, ImageProviderSubmission, ImageVendorId,
    NormalizedProviderGenerationResult,
};

struct FakeImageProvider {
    descriptor: ImageGenerationProviderDescriptor,
}

#[async_trait::async_trait]
impl ImageGenerationProvider for FakeImageProvider {
    fn with_caller(
        &self,
        _caller: &sdkwork_utils_rust::caller_identity::CallerCredentials,
    ) -> Arc<dyn ImageGenerationProvider> {
        Arc::new(FakeImageProvider {
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
        command: &ImageGenerationCommand,
    ) -> ImageGenerationProviderResult<ImageProviderSubmission> {
        let mut dispatch_plan = plan_unified_image_generation_provider_dispatch(command)
            .expect("provider dispatch plan");
        dispatch_plan.provider_id = self.descriptor.id.clone();
        Ok(ImageProviderSubmission {
            dispatch_plan,
            result: submitted_result(command.vendor.as_str(), "task-001"),
        })
    }

    async fn retrieve(
        &self,
        dispatch_plan: &ImageProviderDispatchPlan,
        provider_task_id: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        Ok(submitted_result(
            &dispatch_plan.provider_code,
            provider_task_id,
        ))
    }
}

#[tokio::test]
async fn unified_service_routes_generate_and_retrieve_through_injected_spi() {
    let provider = Arc::new(FakeImageProvider {
        descriptor: ImageGenerationProviderDescriptor {
            id: "fake-image-provider".to_string(),
            vendors: vec![ImageVendorId::new("openai").expect("vendor")],
            capabilities: Vec::new(),
        },
    });
    let registry = ImageGenerationProviderRegistry::builder()
        .register(provider)
        .expect("provider")
        .default_provider("fake-image-provider")
        .build()
        .expect("registry");
    let service = ImageGenerationService::new(registry);
    let command = ImageGenerationCommand {
        vendor: ImageVendorId::new("openai").expect("vendor"),
        model: ImageGenerationModelSelection::named("gpt-image-1").expect("model"),
        prompt: "Product hero".to_string(),
        negative_prompt: None,
        scene: "product_hero".to_string(),
        geometry: Some(ImageGenerationGeometry::Dimensions {
            width: 1024,
            height: 1024,
        }),
        quality: None,
        style: None,
        output_count: 1,
        reference_images: Vec::new(),
        callback_url: None,
        idempotency_key: Some("idem-001".to_string()),
        vendor_parameters: None,
    };

    let submission = service.generate(command).await.expect("submission");
    assert_eq!(submission.dispatch_plan.provider_id, "fake-image-provider");
    assert_eq!(
        submission.result.provider_task_id.as_deref(),
        Some("task-001")
    );

    let retrieved = service
        .retrieve(&submission.dispatch_plan, "task-001")
        .await
        .expect("retrieve");
    assert_eq!(retrieved.provider_code, "openai");
}

fn submitted_result(vendor: &str, task_id: &str) -> NormalizedProviderGenerationResult {
    NormalizedProviderGenerationResult {
        provider_code: vendor.to_string(),
        provider_task_id: Some(task_id.to_string()),
        provider_status: Some("submitted".to_string()),
        provider_state: None,
        status: ImageGenerationRuntimeStatus::Submitted,
        provider_terminal: false,
        ready_for_drive_import: false,
        outputs: Vec::new(),
        error_code: None,
        error_message: None,
    }
}
