use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use sdkwork_iam_context_service::IamAppContext;
use sdkwork_image_generation_host::{
    ImageEditTaskCreateCommand, ImageGalleryItemCreateCommand, ImageGenerationCancelCommandWire,
    ImageGenerationCommandWire, ImageGenerationHost, ImageGenerationRefreshCommandWire,
    ImageOperationCommandWire,
};
use sdkwork_web_core::WebRequestContext;
use serde::Deserialize;

use crate::api_response::{map_catalog_error, map_service_error, success_item, success_items};
use crate::subject::runtime_subject_from_extension;

#[derive(Clone)]
struct AppState {
    host: Arc<ImageGenerationHost>,
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    page: Option<i64>,
    page_size: Option<i64>,
    q: Option<String>,
}

pub fn build_image_app_router(host: Arc<ImageGenerationHost>) -> Router {
    Router::new()
        .route("/app/v3/api/image/presets", get(list_presets))
        .route("/app/v3/api/image/presets/{presetId}", get(retrieve_preset))
        .route(
            "/app/v3/api/image/generations",
            get(list_generations).post(create_generation),
        )
        .route(
            "/app/v3/api/image/generations/{generationId}",
            get(retrieve_generation),
        )
        .route(
            "/app/v3/api/image/generations/{generationId}/refresh",
            post(refresh_generation),
        )
        .route(
            "/app/v3/api/image/generations/{generationId}/cancel",
            post(cancel_generation),
        )
        .route("/app/v3/api/image/edit_tasks", post(create_edit_task))
        .route(
            "/app/v3/api/image/edit_tasks/{taskId}",
            get(retrieve_edit_task),
        )
        .route("/app/v3/api/image/assets", get(list_assets))
        .route("/app/v3/api/image/assets/{assetId}", get(retrieve_asset))
        .route("/app/v3/api/image/galleries", get(list_galleries))
        .route(
            "/app/v3/api/image/galleries/{galleryId}",
            get(retrieve_gallery),
        )
        .route(
            "/app/v3/api/image/galleries/{galleryId}/items",
            post(create_gallery_item),
        )
        .with_state(AppState { host })
}

async fn create_generation(
    State(state): State<AppState>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
    Json(body): Json<ImageGenerationCommandWire>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_service_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageGenerationServiceError::Validation(error),
            )
        }
    };
    match state.host.service().create_generation(&subject, body).await {
        Ok(item) => success_item(context.as_ref().map(|Extension(ctx)| ctx), item),
        Err(error) => map_service_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    }
}

async fn list_generations(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_service_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageGenerationServiceError::Validation(error),
            )
        }
    };
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let items = match state
        .host
        .service()
        .list_generations(&subject, page, page_size)
        .await
    {
        Ok(items) => items,
        Err(error) => return map_service_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    };
    let total = items.len() as i64;
    success_items(
        context.as_ref().map(|Extension(ctx)| ctx),
        items,
        page,
        page_size,
        Some(total),
    )
}

async fn retrieve_generation(
    State(state): State<AppState>,
    Path(generation_id): Path<String>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_service_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageGenerationServiceError::Validation(error),
            )
        }
    };
    match state
        .host
        .service()
        .get_generation(&subject, generation_id.trim())
        .await
    {
        Ok(item) => success_item(context.as_ref().map(|Extension(ctx)| ctx), item),
        Err(error) => map_service_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    }
}

async fn refresh_generation(
    State(state): State<AppState>,
    Path(generation_id): Path<String>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
    Json(body): Json<ImageGenerationRefreshCommandWire>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_service_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageGenerationServiceError::Validation(error),
            )
        }
    };
    match state
        .host
        .service()
        .refresh_generation(&subject, generation_id.trim(), body)
        .await
    {
        Ok(item) => success_item(context.as_ref().map(|Extension(ctx)| ctx), item),
        Err(error) => map_service_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    }
}

async fn cancel_generation(
    State(state): State<AppState>,
    Path(generation_id): Path<String>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
    Json(body): Json<ImageGenerationCancelCommandWire>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_service_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageGenerationServiceError::Validation(error),
            )
        }
    };
    match state
        .host
        .service()
        .cancel_generation(&subject, generation_id.trim(), body.reason)
        .await
    {
        Ok(item) => success_item(context.as_ref().map(|Extension(ctx)| ctx), item),
        Err(error) => map_service_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    }
}

async fn list_presets(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_catalog_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(error),
            )
        }
    };
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 200);
    let items = match state
        .host
        .catalog()
        .list_presets(&subject, page, page_size, query.q)
        .await
    {
        Ok(items) => items,
        Err(error) => return map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    };
    success_items(
        context.as_ref().map(|Extension(ctx)| ctx),
        items,
        page,
        page_size,
        None,
    )
}

async fn retrieve_preset(
    State(state): State<AppState>,
    Path(preset_id): Path<String>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_catalog_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(error),
            )
        }
    };
    match state
        .host
        .catalog()
        .get_preset(&subject, preset_id.trim())
        .await
    {
        Ok(item) => success_item(context.as_ref().map(|Extension(ctx)| ctx), item),
        Err(error) => map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    }
}

async fn list_assets(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_catalog_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(error),
            )
        }
    };
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 200);
    let items = match state
        .host
        .catalog()
        .list_assets(&subject, page, page_size, query.q)
        .await
    {
        Ok(items) => items,
        Err(error) => return map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    };
    success_items(
        context.as_ref().map(|Extension(ctx)| ctx),
        items,
        page,
        page_size,
        None,
    )
}

async fn retrieve_asset(
    State(state): State<AppState>,
    Path(asset_id): Path<String>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_catalog_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(error),
            )
        }
    };
    match state
        .host
        .catalog()
        .get_asset(&subject, asset_id.trim())
        .await
    {
        Ok(item) => success_item(context.as_ref().map(|Extension(ctx)| ctx), item),
        Err(error) => map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    }
}

async fn list_galleries(
    State(state): State<AppState>,
    Query(query): Query<ListQuery>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_catalog_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(error),
            )
        }
    };
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 200);
    let items = match state
        .host
        .catalog()
        .list_galleries(&subject, page, page_size, query.q)
        .await
    {
        Ok(items) => items,
        Err(error) => return map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    };
    success_items(
        context.as_ref().map(|Extension(ctx)| ctx),
        items,
        page,
        page_size,
        None,
    )
}

async fn retrieve_gallery(
    State(state): State<AppState>,
    Path(gallery_id): Path<String>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_catalog_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(error),
            )
        }
    };
    match state
        .host
        .catalog()
        .get_gallery(&subject, gallery_id.trim())
        .await
    {
        Ok(item) => success_item(context.as_ref().map(|Extension(ctx)| ctx), item),
        Err(error) => map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    }
}

async fn create_gallery_item(
    State(state): State<AppState>,
    Path(gallery_id): Path<String>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
    Json(body): Json<ImageOperationCommandWire>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_catalog_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(error),
            )
        }
    };
    let asset_id = body
        .asset_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            sdkwork_image_generation_host::ImageCatalogServiceError::Validation(
                "assetId is required".to_string(),
            )
        });
    let asset_id = match asset_id {
        Ok(value) => value,
        Err(error) => return map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    };
    match state
        .host
        .catalog()
        .create_gallery_item(
            &subject,
            gallery_id.trim(),
            ImageGalleryItemCreateCommand {
                asset_id,
                caption: body.caption,
                sort_order: body.sort_order,
            },
        )
        .await
    {
        Ok(item) => success_item(context.as_ref().map(|Extension(ctx)| ctx), item),
        Err(error) => map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    }
}

async fn create_edit_task(
    State(state): State<AppState>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
    Json(body): Json<ImageOperationCommandWire>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_catalog_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(error),
            )
        }
    };
    let command = match map_edit_task_command(body) {
        Ok(command) => command,
        Err(error) => return map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    };
    match state
        .host
        .catalog()
        .create_edit_task(&subject, command)
        .await
    {
        Ok(item) => success_item(context.as_ref().map(|Extension(ctx)| ctx), item),
        Err(error) => map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    }
}

async fn retrieve_edit_task(
    State(state): State<AppState>,
    Path(task_id): Path<String>,
    context: Option<Extension<WebRequestContext>>,
    iam: Option<Extension<IamAppContext>>,
) -> Response {
    let subject = match runtime_subject_from_extension(iam) {
        Ok(subject) => subject,
        Err(error) => {
            return map_catalog_error(
                context.as_ref().map(|Extension(ctx)| ctx),
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(error),
            )
        }
    };
    match state
        .host
        .catalog()
        .get_edit_task(&subject, task_id.trim())
        .await
    {
        Ok(item) => success_item(context.as_ref().map(|Extension(ctx)| ctx), item),
        Err(error) => map_catalog_error(context.as_ref().map(|Extension(ctx)| ctx), error),
    }
}

fn map_edit_task_command(
    body: ImageOperationCommandWire,
) -> Result<ImageEditTaskCreateCommand, sdkwork_image_generation_host::ImageCatalogServiceError> {
    Ok(ImageEditTaskCreateCommand {
        source_asset_id: body
            .source_asset_id
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(
                    "sourceAssetId is required".to_string(),
                )
            })?,
        edit_type: body
            .edit_type
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(
                    "editType is required".to_string(),
                )
            })?,
        prompt: body
            .prompt
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                sdkwork_image_generation_host::ImageCatalogServiceError::Validation(
                    "prompt is required".to_string(),
                )
            })?,
        negative_prompt: body.negative_prompt,
    })
}
