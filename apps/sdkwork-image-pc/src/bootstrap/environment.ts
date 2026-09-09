import { resolveBaseUrlWithAlignProtocol } from "@sdkwork/sdk-common";

export type Environment = "development" | "test" | "staging" | "production";
export type DeploymentMode = "saas" | "private" | "local" | "test";

export interface RuntimeEnvironment {
  environment: Environment;
  deploymentMode: DeploymentMode;
  apiBaseUrl: string;
  appBaseUrl: string;
}

export function resolveEnvironment(): RuntimeEnvironment {
  const env = (import.meta.env.VITE_ENVIRONMENT as Environment) ?? "development";
  return {
    environment: env,
    deploymentMode: env === "development" ? "local" : "saas",
    // Single-call §6.3 resolution: explicit Vite overrides win as candidates
    // and the returned origins always follow the page scheme.
    apiBaseUrl: resolveBaseUrlWithAlignProtocol({
      baseUrls: import.meta.env.VITE_API_BASE_URL || undefined,
    }).url,
    appBaseUrl: resolveBaseUrlWithAlignProtocol({
      baseUrls: import.meta.env.VITE_APP_BASE_URL || undefined,
    }).url,
  };
}
