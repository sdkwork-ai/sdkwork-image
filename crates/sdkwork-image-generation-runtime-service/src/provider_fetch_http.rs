use async_trait::async_trait;
use reqwest::Url;

use crate::provider_fetch::{
    ProviderArtifactContent, ProviderArtifactFetcher, ProviderArtifactRef,
};

#[derive(Clone, Debug)]
pub struct HttpProviderArtifactFetcher {
    client: reqwest::Client,
}

impl HttpProviderArtifactFetcher {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { client })
    }

    fn resolve_fetch_url(artifact: &ProviderArtifactRef) -> Result<Url, String> {
        let provider_url = artifact
            .provider_url
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let provider_uri = artifact
            .provider_uri
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let raw = provider_url
            .or(provider_uri)
            .ok_or_else(|| "provider artifact url is required".to_string())?;
        let url =
            Url::parse(raw).map_err(|error| format!("invalid provider artifact url: {error}"))?;
        match url.scheme() {
            "http" | "https" => Ok(url),
            _ => Err("provider artifact url must use http or https".to_string()),
        }
    }
}

#[async_trait]
impl ProviderArtifactFetcher for HttpProviderArtifactFetcher {
    async fn fetch_provider_artifact(
        &self,
        artifact: &ProviderArtifactRef,
    ) -> Result<ProviderArtifactContent, String> {
        let url = Self::resolve_fetch_url(artifact)?;
        let response = self
            .client
            .get(url.clone())
            .send()
            .await
            .map_err(|error| format!("provider artifact fetch failed: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "provider artifact fetch returned HTTP {}",
                response.status().as_u16()
            ));
        }
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let body = response
            .bytes()
            .await
            .map_err(|error| format!("provider artifact body read failed: {error}"))?
            .to_vec();
        let original_file_name = artifact
            .file_name
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("output-{}.bin", artifact.output_index));
        Ok(ProviderArtifactContent {
            output_index: artifact.output_index,
            body,
            content_type,
            original_file_name,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_http_provider_urls() {
        let artifact = ProviderArtifactRef {
            output_index: 0,
            provider_url: Some("file:///etc/passwd".to_string()),
            provider_uri: None,
            file_name: None,
            mime_type: None,
        };
        assert!(HttpProviderArtifactFetcher::resolve_fetch_url(&artifact).is_err());
    }

    #[test]
    fn accepts_https_provider_urls() {
        let artifact = ProviderArtifactRef {
            output_index: 0,
            provider_url: Some("https://provider.example.com/a.png".to_string()),
            provider_uri: None,
            file_name: None,
            mime_type: None,
        };
        assert!(HttpProviderArtifactFetcher::resolve_fetch_url(&artifact).is_ok());
    }
}
