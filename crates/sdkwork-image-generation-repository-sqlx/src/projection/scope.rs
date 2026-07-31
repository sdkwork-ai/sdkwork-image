use sdkwork_image_generation_service::ImageGenerationActor;

use crate::RepositoryError;

pub fn parse_scope_id(value: &str, field: &'static str) -> Result<i64, RepositoryError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(RepositoryError::Validation(format!("{field} is required")));
    }
    value
        .parse::<i64>()
        .map_err(|_| RepositoryError::Validation(format!("{field} must be a numeric identifier")))
}

pub fn actor_user_id(actor: &ImageGenerationActor) -> Result<i64, RepositoryError> {
    match actor {
        ImageGenerationActor::User { user_id } => parse_scope_id(user_id, "user_id"),
        ImageGenerationActor::Anonymous { .. } => Ok(0),
        ImageGenerationActor::System { operator_id } => parse_scope_id(operator_id, "operator_id"),
    }
}

pub fn organization_id(
    scope: &sdkwork_image_generation_workflow_service::ImageGenerationScope,
) -> Result<i64, RepositoryError> {
    match scope.organization_id.as_deref() {
        Some(value) if !value.trim().is_empty() => parse_scope_id(value, "organization_id"),
        _ => Ok(0),
    }
}
