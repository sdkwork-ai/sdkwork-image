//! Proves the standalone image host follows the registry target changes
//! without a restart.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use sdkwork_image_generation_provider_adapter::ImageGenerationProviderAdapter;
use sdkwork_utils_rust::service_base_url::{
    install_http_service_registry, HttpServiceRegistry, RegistryHttpTarget,
};

struct SwitchableRegistry {
    second: AtomicBool,
}

impl HttpServiceRegistry for SwitchableRegistry {
    fn resolve_http(&self, service: &str) -> Option<RegistryHttpTarget> {
        if service != "cloudrouter" {
            return None;
        }
        let second = self.second.load(Ordering::SeqCst);
        Some(RegistryHttpTarget {
            service: "cloudrouter".to_owned(),
            instance_id: if second {
                "registry-image-2"
            } else {
                "registry-image-1"
            }
            .to_owned(),
            base_url: if second {
                "https://registry-b.example.com"
            } else {
                "https://registry-a.example.com"
            }
            .to_owned(),
        })
    }

    fn report_failure(&self, _target: &RegistryHttpTarget, _reason: &str) {}

    fn report_success(&self, _target: &RegistryHttpTarget) {}
}

#[test]
fn image_adapter_follows_the_registry_target_and_online_failover() {
    let registry = Arc::new(SwitchableRegistry {
        second: AtomicBool::new(false),
    });
    assert!(
        install_http_service_registry(registry.clone()),
        "the registry slot is process-global and this test binary owns it"
    );

    let adapter =
        ImageGenerationProviderAdapter::from_settings("https://fallback.example.com", None)
            .expect("image adapter builds");

    assert_eq!(adapter.base_url(), "https://registry-a.example.com");

    registry.second.store(true, Ordering::SeqCst);
    assert_eq!(
        adapter.base_url(),
        "https://registry-b.example.com",
        "the standalone image adapter rebuilds its client on target change"
    );
}
