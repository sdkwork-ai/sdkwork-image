# Image Technical Architecture

Status: draft
Owner: SDKWork maintainers
Updated: 2026-06-24
Specs: ARCHITECTURE_DECISION_SPEC.md, DOCUMENTATION_SPEC.md

## Document Map

- Add `TECH-<topic>.md` shards in this directory when the architecture grows beyond one reviewable screen.

## 1. Architecture Overview

Image generation follows the SDKWork L0-L6 dependency direction. HTTP routes call the application
service; the application service calls the unified image generation service; the unified service
selects an injected provider SPI implementation; the default L4 adapter calls the generated unified
Rust SDK. Provider artifacts are normalized before workflow persistence and Drive import.

```text
route -> application service -> ImageGenerationServicePort -> ImageGenerationProvider
                                                       ^
                                                       |
                                      default provider adapter -> generated Rust SDK

agent -> image MCP adapter -> ImageGenerationServicePort

runtime host -> constructs SDK client, adapter, registry, services, repositories, and Drive runtime
```

## 2. Technology Choices

## 3. System Boundaries And Modules

- `sdkwork-image-generation-provider-spi` is L3 `backend-domain`; it contains no SDK client, URL,
  credential, generated DTO, SDK resource, or SDK method.
- `sdkwork-image-generation-service` is L2 `backend-service`; its public root is the normal
  dependency entrypoint for other Rust modules.
- `sdkwork-image-generation-provider-adapter` is L4 `backend-provider`; it owns SDK operation
  routing, SDK DTO conversion, vendor extension schema decoding, and response normalization.
- `sdkwork-image-generation-mcp-service` is an agent-facing `backend-route` protocol adapter. It
  owns MCP DTOs, tools, resources, prompts, error mapping, task handles, and transport builders. It
  depends only on `ImageGenerationServicePort` and a task-context store port.
- `sdkwork-image-generation-host` is L5 `runtime-service-host`; only this layer constructs the
  generated SDK client and registers concrete provider implementations.

## 4. Directory And Package Layout

```text
crates/sdkwork-image-generation-provider-spi/
crates/sdkwork-image-generation-service/
crates/sdkwork-image-generation-mcp-service/
crates/sdkwork-image-generation-provider-adapter/
crates/sdkwork-image-generation-workflow-service/
crates/sdkwork-image-generation-runtime-service/
crates/sdkwork-image-generation-host/
```

## 5. API, SDK, And Data Ownership

The generated CloudRouter open SDK remains dependency-owned. It is consumed directly only by the
provider adapter and runtime composition root; generated files are not edited or copied. Persistent
provider snapshots store provider id, vendor code, semantic operation, model, task mode, common
parameters, and task identifiers. They do not store generated SDK route or method identifiers.

## 6. Security, Privacy, And Observability

## 7. Deployment And Runtime Topology

MCP processes may expose stdio or MCP Streamable HTTP. Streamable HTTP is the network transport and
uses `text/event-stream` for server-to-client delivery; no deprecated standalone SSE transport is
defined. The mounting host owns authentication, authorization, allowed hosts/origins, request and
payload limits, tracing, listener binding, cancellation, graceful shutdown, and any durable task
context store.

## 8. Architecture Decision Index

- [ADR-20260719-image-generation-provider-spi](../decisions/ADR-20260719-image-generation-provider-spi.md)
- [ADR-20260719-media-generation-mcp-services](../decisions/ADR-20260719-media-generation-mcp-services.md)

## 9. Verification

- `cargo test -p sdkwork-image-generation-provider-spi`
- `cargo test -p sdkwork-image-generation-provider-adapter`
- `cargo test -p sdkwork-image-generation-service`
- `cargo test -p sdkwork-image-generation-mcp-service`
- `cargo test -p sdkwork-image-generation-workflow-service`
- `cargo test -p sdkwork-image-generation-runtime-service`
- `cargo test -p sdkwork-image-generation-host`
- SDKWork component-port and application-layering validators from `../sdkwork-specs/tools/`
