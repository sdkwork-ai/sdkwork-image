use axum::Extension;
use sdkwork_iam_context_service::IamAppContext;
use sdkwork_image_generation_host::{runtime_subject_from_iam, RuntimeSubject};

pub fn runtime_subject_from_extension(
    context: Option<Extension<IamAppContext>>,
) -> Result<RuntimeSubject, String> {
    let Some(Extension(context)) = context else {
        return Err("authenticated runtime context is required".to_owned());
    };
    runtime_subject_from_iam(&context)
}
