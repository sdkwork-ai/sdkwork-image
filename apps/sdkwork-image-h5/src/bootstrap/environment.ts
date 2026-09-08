import { resolveBaseUrl } from "@sdkwork/sdk-common";

export type Environment = "development" | "test" | "staging" | "production";

export interface RuntimeEnvironment {
  environment: Environment;
  apiBaseUrl: string;
}

export function resolveEnvironment(): RuntimeEnvironment {
  const env = (import.meta.env.VITE_ENVIRONMENT as Environment) ?? "development";
  return {
    environment: env,
    // Prefer an explicit Vite override; otherwise resolve the shared
    // SDKWORK_API_BASE_URL through @sdkwork/sdk-common (env + brand + protocol
    // aware), eliminating the hardcoded localhost default.
    apiBaseUrl: import.meta.env.VITE_API_BASE_URL ?? resolveBaseUrl().url,
  };
}
