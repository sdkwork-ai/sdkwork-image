import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { test } from "node:test";
import { resolve } from "node:path";

const workspaceRoot = resolve(import.meta.dirname, "..");

function read(relativePath) {
  return readFileSync(resolve(workspaceRoot, relativePath), "utf8");
}

test("image Rust workspace uses standard crates layout and names", () => {
  const rootCargo = read("Cargo.toml");
  const packageJson = JSON.parse(read("package.json"));
  const pnpmWorkspace = read("pnpm-workspace.yaml");
  const materializer = read("sdks/materialize-image-v3-openapi-boundaries.mjs");

  assert.equal(
    rootCargo.includes("packages/native-rust"),
    false,
    "root Cargo.toml must not retain legacy packages/native-rust members or dependency paths",
  );
  assert.equal(
    pnpmWorkspace.includes("packages/native-rust"),
    false,
    "pnpm workspace must not include legacy Rust package globs",
  );
  assert.equal(
    packageJson.scripts["test:rust"],
    "cargo test --workspace",
    "Rust verification must run from the standard Cargo workspace",
  );
  assert.equal(
    materializer.includes("packages/native-rust"),
    false,
    "OpenAPI materializer must scan standard crates/ route sources",
  );

  for (const expected of [
    "crates/sdkwork-image-generation-service",
    "crates/sdkwork-image-generation-provider-spi",
    "crates/sdkwork-image-generation-provider-adapter",
    "crates/sdkwork-image-generation-workflow-service",
    "crates/sdkwork-image-generation-repository-sqlx",
    "crates/sdkwork-routes-image-open-api",
    "crates/sdkwork-routes-image-app-api",
    "crates/sdkwork-routes-image-backend-api",
  ]) {
    assert.match(rootCargo, new RegExp(expected.replaceAll("/", "[/\\\\]")));
    assert.equal(existsSync(resolve(workspaceRoot, expected, "Cargo.toml")), true, `missing ${expected}`);
  }

  for (const forbidden of [
    "sdkwork_image_core",
    "sdkwork_image_service",
    "sdkwork_image_provider_cloud_router",
    "sdkwork_image_storage_sqlx",
    "sdkwork_image_http",
    "sdkwork-image-core-rust",
    "sdkwork-image-service-rust",
    "sdkwork-image-provider-cloud-router-rust",
    "sdkwork-image-cloud-router-provider-service",
    "sdkwork-image-storage-sqlx-rust",
    "sdkwork-image-http-rust",
  ]) {
    assert.equal(rootCargo.includes(forbidden), false, `root Cargo.toml retains ${forbidden}`);
  }
});
