use sdkwork_image_generation_runtime_service::plan_drive_upload_preparations;
use sdkwork_image_generation_service::{
    plan_drive_import_for_generated_outputs, DriveGeneratedMediaContext, GeneratedMediaKind,
    GeneratedMediaOutput, ImageGenerationActor, IMAGE_WORKSPACE,
};

#[test]
fn plans_drive_upload_preparations_from_image_drive_import_plans() {
    let context = DriveGeneratedMediaContext {
        tenant_id: "100001".to_string(),
        organization_id: None,
        generation_id: "generation-001".to_string(),
        provider_code: "openai".to_string(),
        model: Some("gpt-image-1".to_string()),
        scene: "product_hero".to_string(),
        actor: ImageGenerationActor::User {
            user_id: "user-001".to_string(),
        },
    };
    let outputs = vec![GeneratedMediaOutput {
        output_index: 0,
        kind: GeneratedMediaKind::Image,
        provider_asset_id: None,
        provider_uri: None,
        provider_url: Some("https://provider.example.com/temporary-image.png".to_string()),
        file_name: Some("hero.png".to_string()),
        mime_type: Some("image/png".to_string()),
        size_bytes: Some(2048),
        width: Some(1024),
        height: Some(1024),
        duration_seconds: None,
    }];
    let image_plans =
        plan_drive_import_for_generated_outputs(context.clone(), outputs.clone()).expect("plans");
    let (_, preparations) = plan_drive_upload_preparations(
        context.tenant_id,
        context.organization_id,
        &context.actor,
        &image_plans,
        &outputs,
        "text_to_image",
        "operator-001",
        1_780_000_000_000,
    )
    .expect("preparations");

    assert_eq!(preparations.len(), 1);
    assert_eq!(preparations[0].output_index, 0);
    assert_eq!(preparations[0].prepare.app_id, IMAGE_WORKSPACE);
    assert_eq!(preparations[0].prepare.upload_profile_code, "image");
    assert_eq!(preparations[0].prepare.app_resource_id, "generation-001:0");
}
