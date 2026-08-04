use std::sync::Arc;

use async_trait::async_trait;
use axum::{body::Body, http::Request, Router};
use rmcp::{transport::streamable_http_server::StreamableHttpServerConfig, ServerHandler};
use sdkwork_image_generation_mcp_service::{
    streamable_http_service, GenerateImageInput, ImageGenerationMcpService,
};
use sdkwork_image_generation_service::{
    ImageGenerationCommand, ImageGenerationProviderDescriptor, ImageGenerationProviderResult,
    ImageGenerationServicePort, ImageProviderDispatchPlan, ImageProviderSubmission,
    NormalizedProviderGenerationResult,
};
use tower::ServiceExt;

struct FakeImageGenerationService;

#[async_trait]
impl ImageGenerationServicePort for FakeImageGenerationService {
    async fn generate(
        &self,
        _: ImageGenerationCommand,
    ) -> ImageGenerationProviderResult<ImageProviderSubmission> {
        unreachable!()
    }
    async fn retrieve(
        &self,
        _: &ImageProviderDispatchPlan,
        _: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        unreachable!()
    }
    async fn cancel(
        &self,
        _: &ImageProviderDispatchPlan,
        _: &str,
    ) -> ImageGenerationProviderResult<NormalizedProviderGenerationResult> {
        unreachable!()
    }
    fn provider_descriptors(&self) -> Vec<ImageGenerationProviderDescriptor> {
        Vec::new()
    }
}

fn service() -> ImageGenerationMcpService {
    ImageGenerationMcpService::new(Arc::new(FakeImageGenerationService))
}

#[test]
fn public_contract_is_provider_neutral() {
    let service = service();
    let names = service
        .tools()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "image.cancel",
            "image.capabilities",
            "image.generate",
            "image.retrieve"
        ]
    );
    let contract = serde_json::to_string(&service.tools())
        .expect("serialize tools")
        .to_ascii_lowercase();
    for forbidden in [
        "cloudrouter",
        "open-sdk",
        "generated/server-openapi",
        "provider_operation",
        "providerid",
    ] {
        assert!(
            !contract.contains(forbidden),
            "public MCP contract leaked {forbidden}"
        );
    }
    let info = service.get_info();
    assert!(info.capabilities.tools.is_some());
    assert!(info.capabilities.resources.is_some());
    assert!(info.capabilities.prompts.is_some());
}

#[tokio::test]
async fn streamable_http_initialize_uses_sse() {
    let app = Router::new().nest_service(
        "/mcp",
        streamable_http_service(service(), StreamableHttpServerConfig::default()),
    );
    let response = app
        .oneshot(
            Request::post("/mcp")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .body(Body::from(initialize_body()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert!(response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap()
        .contains("text/event-stream"));
}

fn initialize_body() -> &'static str {
    r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"sdkwork-contract-test","version":"1.0.0"}}}"#
}

#[test]
fn unified_input_maps_vendor_extensions_without_sdk_dtos() {
    let input: GenerateImageInput = serde_json::from_value(serde_json::json!({
        "vendor": "Example_Vendor",
        "prompt": "a precise product image",
        "vendorParameters": { "schema": "urn:example:image:v1", "values": { "seed": 7 } }
    }))
    .unwrap();
    let command: ImageGenerationCommand = input.try_into().unwrap();
    assert_eq!(command.vendor.as_str(), "example-vendor");
    assert_eq!(
        command.vendor_parameters.unwrap().schema,
        "urn:example:image:v1"
    );
}
