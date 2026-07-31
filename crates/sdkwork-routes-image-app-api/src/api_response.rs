use axum::http::{HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use sdkwork_image_generation_host::{ImageCatalogServiceError, ImageGenerationServiceError};
use sdkwork_utils_rust::{
    PageInfo, PageMode, SdkWorkApiResponse, SdkWorkPageData, SdkWorkProblemDetail,
    SdkWorkResourceData, SdkWorkResultCode, SDKWORK_TRACE_ID_HEADER,
};
use sdkwork_web_core::WebRequestContext;

pub fn resolve_trace_id(context: Option<&WebRequestContext>) -> String {
    context
        .and_then(|ctx| ctx.trace_id.clone())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(sdkwork_utils_rust::uuid)
}

pub fn success_item<T: serde::Serialize>(context: Option<&WebRequestContext>, item: T) -> Response {
    let trace_id = resolve_trace_id(context);
    let envelope = SdkWorkApiResponse::success(SdkWorkResourceData { item }, trace_id.clone());
    attach_trace_header((StatusCode::OK, Json(envelope)).into_response(), &trace_id)
}

pub fn success_items<T: serde::Serialize>(
    context: Option<&WebRequestContext>,
    items: Vec<T>,
    page: i64,
    page_size: i64,
    total_items: Option<i64>,
) -> Response {
    let trace_id = resolve_trace_id(context);
    let envelope = SdkWorkApiResponse::success(
        SdkWorkPageData {
            items,
            page_info: PageInfo {
                mode: PageMode::Offset,
                page: Some(page as i32),
                page_size: Some(page_size as i32),
                total_items: total_items.map(|value| value.to_string()),
                total_pages: None,
                next_cursor: None,
                has_more: None,
            },
        },
        trace_id.clone(),
    );
    attach_trace_header((StatusCode::OK, Json(envelope)).into_response(), &trace_id)
}

pub fn map_service_error(
    context: Option<&WebRequestContext>,
    error: ImageGenerationServiceError,
) -> Response {
    let trace_id = resolve_trace_id(context);
    let (status, result_code) = match error {
        ImageGenerationServiceError::Validation(_) => {
            (StatusCode::BAD_REQUEST, SdkWorkResultCode::ValidationError)
        }
        ImageGenerationServiceError::NotFound => {
            (StatusCode::NOT_FOUND, SdkWorkResultCode::NotFound)
        }
        ImageGenerationServiceError::Dispatch(_)
        | ImageGenerationServiceError::Planning(_)
        | ImageGenerationServiceError::DriveImport(_) => {
            (StatusCode::BAD_GATEWAY, SdkWorkResultCode::BadGateway)
        }
        ImageGenerationServiceError::Conflict(_) => {
            (StatusCode::CONFLICT, SdkWorkResultCode::Conflict)
        }
        ImageGenerationServiceError::Persistence(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            SdkWorkResultCode::InternalError,
        ),
    };
    let problem = SdkWorkProblemDetail::platform(result_code, error.to_string(), trace_id.clone());
    attach_trace_header((status, Json(problem)).into_response(), &trace_id)
}

pub fn map_catalog_error(
    context: Option<&WebRequestContext>,
    error: ImageCatalogServiceError,
) -> Response {
    let trace_id = resolve_trace_id(context);
    let (status, result_code) = match error {
        ImageCatalogServiceError::Validation(_) => {
            (StatusCode::BAD_REQUEST, SdkWorkResultCode::ValidationError)
        }
        ImageCatalogServiceError::NotFound => (StatusCode::NOT_FOUND, SdkWorkResultCode::NotFound),
        ImageCatalogServiceError::Persistence(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            SdkWorkResultCode::InternalError,
        ),
    };
    let problem = SdkWorkProblemDetail::platform(result_code, error.to_string(), trace_id.clone());
    attach_trace_header((status, Json(problem)).into_response(), &trace_id)
}

fn attach_trace_header(mut response: Response, trace_id: &str) -> Response {
    if let Ok(value) = HeaderValue::from_str(trace_id) {
        if let Ok(name) = HeaderName::from_bytes(SDKWORK_TRACE_ID_HEADER.as_bytes()) {
            response.headers_mut().insert(name, value);
        }
    }
    response
}
