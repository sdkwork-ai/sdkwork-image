# sdkwork-routes-image-app-api

Image app-api route crate for `/app/v3/api/image/*`.

## Responsibilities

- Route catalog (`manifest.rs`) aligned with OpenAPI authority
- Dual-token HTTP route manifest for web-framework auth/rate-limit resolution
- Generation handlers: create, list, retrieve, refresh, cancel (`routes.rs`)
- Catalog handlers: presets, assets, galleries, edit_tasks (`routes.rs`)
- SdkWorkApiResponse / ProblemDetail mapping via `sdkwork-utils-rust` (`api_response.rs`)
- IAM runtime subject projection from `IamAppContext` (`subject.rs`)

## Bootstrap

Gateway assembly injects `Arc<ImageGenerationHost>` and optionally starts the background processor:

```rust
// Production: CloudRouter + IMAGE database + optional DRIVE import + background processor
let assembly = assemble_api_router_from_env().await?;
// assembly.contribution is consumed intact by the selected gateway profile.
// assembly.background_processor holds the task when IMAGE_BACKGROUND_PROCESSOR_ENABLED (default true).

// Embedded gateways inject their process-shared database pool.
let assembly = assemble_api_router_with_pool(database_pool).await?;
```

The assembly mounts the unwrapped business router. The selected standalone or cloud gateway merges
its route manifest, OpenAPI, permission catalog, domain injectors, and readiness check before
installing Web Framework infrastructure once for the process.

Machine-readable contract: `specs/component.spec.json`. Standards: `../../../../sdkwork-specs/`.
