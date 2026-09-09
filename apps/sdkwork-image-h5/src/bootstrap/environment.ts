import { resolveBaseUrlWithAlignProtocol } from "@sdkwork/sdk-common";

export type Environment = "development" | "test" | "staging" | "production";

export interface RuntimeEnvironment {
  environment: Environment;
  apiBaseUrl: string;
}

export function resolveEnvironment(): RuntimeEnvironment {
  const env = (import.meta.env.VITE_ENVIRONMENT as Environment) ?? "development";
  return {
    environment: env,
    // Single-call §6.3 resolution: the explicit Vite override wins as a
    // candidate and the returned origin always follows the page scheme.
    apiBaseUrl: resolveBaseUrlWithAlignProtocol({
      baseUrls: import.meta.env.VITE_API_BASE_URL || undefined,
    }).url,
  };
}
