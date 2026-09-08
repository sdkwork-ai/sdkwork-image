import { resolveBaseUrl } from "@sdkwork/sdk-common";

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
    // Prefer explicit Vite overrides; otherwise resolve the shared
    // SDKWORK_API_BASE_URL through @sdkwork/sdk-common (env + brand + protocol
    // aware), eliminating the hardcoded localhost defaults.
    apiBaseUrl: import.meta.env.VITE_API_BASE_URL ?? resolveBaseUrl().url,
    appBaseUrl: import.meta.env.VITE_APP_BASE_URL ?? resolveBaseUrl().url,
  };
}
