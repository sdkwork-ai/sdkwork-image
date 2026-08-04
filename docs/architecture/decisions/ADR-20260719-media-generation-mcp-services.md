# ADR-20260719: Independent Media Generation MCP Services

Status: accepted

## Context

Image, video, voice, music, and sound-effect generation already expose provider-neutral application
service ports. Intelligent agents need the same capabilities over Model Context Protocol without
coupling public contracts to a provider router, generated SDK, HTTP route, or deployment process.
The services must support local stdio clients and network clients that consume SSE delivery.

## Decision

Each media domain owns one independently versioned Rust MCP protocol-adapter crate:

| Domain | Package | Service dependency |
| --- | --- | --- |
| Image | `sdkwork-image-generation-mcp-service` | `ImageGenerationServicePort` |
| Video | `sdkwork-video-generation-mcp-service` | `VideoGenerationServicePort` |
| Voice | `sdkwork-voice-generation-mcp-service` | `VoiceGenerationServicePort` |
| Music | `sdkwork-music-generation-mcp-service` | `MusicGenerationServicePort` |
| Sound effect | `sdkwork-audio-sound-effect-mcp-service` | `SoundEffectGenerationServicePort` |

Each crate owns MCP request/response DTOs, stable dotted tool names, capability resources, one
generation prompt, structured tool-error mapping, stdio serving, and a mountable Streamable HTTP
service. Asynchronous domains also own a task-context store port and a bounded in-memory default;
hosts may inject durable implementations without changing MCP tools.

The implementation uses the official Rust MCP SDK `rmcp`. Network delivery uses MCP Streamable
HTTP, whose stateful response channel is SSE (`text/event-stream`). A separate legacy SSE endpoint
is not introduced.

The MCP crates must not depend on provider adapters, generated SDKs, repositories, SQLx, HTTP route
crates, gateway assemblies, or raw provider clients. They must not expose CloudRouter, generated SDK
method names, provider URLs, credentials, or adapter dispatch plans in MCP schemas.

Composition roots mount the service and own authentication, authorization, tenant/user context,
allowed host and origin policy, payload and concurrency limits, observability, listener lifecycle,
graceful shutdown, and production task-context persistence.

## Tool Naming

Tool names use `<domain>.<action>` with lower snake case only where the canonical domain contains
multiple words:

- `image.generate`, `image.retrieve`, `image.cancel`, `image.capabilities`
- `video.generate`, `video.retrieve`, `video.cancel`, `video.capabilities`
- `voice.synthesize`, `voice.retrieve`, `voice.cancel`, `voice.capabilities`
- `music.generate`, `music.retrieve`, `music.cancel`, `music.capabilities`
- `sound_effect.generate`, `sound_effect.capabilities`

Sound effects are currently synchronous, so retrieval and cancellation are intentionally absent.
Tools may only be added when the underlying application service port owns the capability.

## Consequences

- Agent contracts remain stable while provider adapters and generated SDK routes evolve.
- Every domain can be embedded independently in a desktop agent, CLI, gateway, or cloud host.
- Streamable HTTP session policy and security remain configurable at the composition boundary.
- MCP contract tests can verify tool vocabulary, capabilities, and SSE initialization per module.
- A durable production task store is a host choice rather than a hidden MCP dependency.

## Verification

Each package runs `cargo test -p <package>`. Its contract test verifies the exact tool set, checks
that forbidden provider-router vocabulary is absent, validates tool/resource/prompt capabilities,
and initializes Streamable HTTP with a `text/event-stream` response.
