use sdkwork_iam_context_service::IamAppContext;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeSubject {
    pub tenant_id: String,
    pub organization_id: Option<String>,
    pub user_id: String,
}

pub fn runtime_subject_from_iam(context: &IamAppContext) -> Result<RuntimeSubject, String> {
    let tenant_id = required_text(&context.tenant_id, "tenant_id")?;
    let user_id = required_text(&context.user_id, "user_id")?;
    let organization_id = context
        .organization_id
        .as_deref()
        .map(str::trim)
        .filter(|value: &&str| !value.is_empty())
        .map(str::to_owned);
    Ok(RuntimeSubject {
        tenant_id,
        organization_id,
        user_id,
    })
}

fn required_text(value: &str, field_name: &'static str) -> Result<String, String> {
    let value = value.trim();
    if value.is_empty() {
        return Err(format!(
            "authenticated runtime context {field_name} is required"
        ));
    }
    Ok(value.to_owned())
}
