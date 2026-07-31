# SDKWork Image Generation Host

L5 runtime composition for image generation. The host constructs the generated dependency SDK
client, registers the default provider adapter, builds the unified image generation service, and
injects repository and Drive runtimes into the application service.

Standalone hosts can bootstrap from runtime configuration with
`ImageGenerationHost::from_runtime_env`. Embedded gateways use
`ImageGenerationHost::from_pool` so the API assembly shares the process database pool and exposes
the same pool through its readiness contribution.
