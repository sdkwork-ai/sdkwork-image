use cloudrouter_open_sdk::{
    ProviderGeneratedMedia, ProviderTaskError, ViduCreation, ViduImageGenerationTask,
    ViduTaskCreationsResponse,
};
use sdkwork_image_generation_provider_spi::{
    normalize_provider_task_generation_result, NormalizedProviderGenerationResult,
    ProviderGeneratedMediaAsset, ProviderTaskErrorSnapshot, ProviderTaskSnapshot,
};

pub fn normalize_vidu_image_generation_task_result(
    provider_code: impl AsRef<str>,
    task: ViduImageGenerationTask,
) -> Result<NormalizedProviderGenerationResult, &'static str> {
    normalize_provider_task_generation_result(
        provider_code,
        ProviderTaskSnapshot {
            task_id: task.task_id,
            id: None,
            status: None,
            state: task.state,
            model: task.model,
            images: vidu_assets_from_creations(task.creations.unwrap_or_default()),
            error: None,
        },
    )
}

pub fn normalize_vidu_task_creations_result(
    provider_code: impl AsRef<str>,
    task: ViduTaskCreationsResponse,
) -> Result<NormalizedProviderGenerationResult, &'static str> {
    normalize_provider_task_generation_result(
        provider_code,
        ProviderTaskSnapshot {
            task_id: task.task_id,
            id: None,
            status: None,
            state: task.state,
            model: task.model,
            images: vidu_assets_from_creations(task.creations.unwrap_or_default()),
            error: None,
        },
    )
}

pub(crate) fn provider_assets_from_generated_media(
    assets: Vec<ProviderGeneratedMedia>,
) -> Vec<ProviderGeneratedMediaAsset> {
    assets
        .into_iter()
        .map(|asset| ProviderGeneratedMediaAsset {
            id: asset.id,
            uri: asset.uri,
            url: asset.url,
            mime_type: asset.mime_type,
            width: asset.width,
            height: asset.height,
            duration_seconds: asset.duration.map(|value| value.round() as i32),
        })
        .collect()
}

pub(crate) fn provider_error_from_sdk(error: ProviderTaskError) -> ProviderTaskErrorSnapshot {
    ProviderTaskErrorSnapshot {
        code: error.code,
        message: error.message,
        error_type: error.r#type,
    }
}

fn vidu_assets_from_creations(creations: Vec<ViduCreation>) -> Vec<ProviderGeneratedMediaAsset> {
    creations
        .into_iter()
        .map(|creation| ProviderGeneratedMediaAsset {
            id: creation.id,
            uri: creation.uri,
            url: creation.image_url,
            mime_type: None,
            width: creation.width,
            height: creation.height,
            duration_seconds: None,
        })
        .collect()
}
