import { createSdkworkBackdropStyle, createSdkworkHeroStyle, createSdkworkPanelStyle, createSdkworkToneStyle, type SdkworkThemeVisualTone } from "@sdkwork/ui-pc-react/theme";

export type SdkworkGenerationVisualTone = SdkworkThemeVisualTone;
export type SdkworkGenerationStyle = Record<string, number | string | undefined>;

function toSdkworkGenerationStyle(style: unknown): SdkworkGenerationStyle {
  return style as SdkworkGenerationStyle;
}

export function createSdkworkGenerationToneStyle(
  tone: SdkworkGenerationVisualTone,
  options: {
    backgroundWeight?: number;
    borderWeight?: number;
  } = {},
): SdkworkGenerationStyle {
  return toSdkworkGenerationStyle(createSdkworkToneStyle(tone, options));
}

export function createSdkworkGenerationPanelStyle(
  tone: SdkworkGenerationVisualTone,
  options: {
    backgroundWeight?: number;
    borderWeight?: number;
    surfaceColor?: string;
    surfaceWeight?: number;
  } = {},
): SdkworkGenerationStyle {
  return toSdkworkGenerationStyle(createSdkworkPanelStyle(tone, options));
}

export function createSdkworkGenerationBackdropStyle(): SdkworkGenerationStyle {
  return toSdkworkGenerationStyle(createSdkworkBackdropStyle());
}

export function createSdkworkGenerationHeroStyle(): SdkworkGenerationStyle {
  return toSdkworkGenerationStyle(createSdkworkHeroStyle());
}

export function createSdkworkGenerationHeroTextStyle(
  tone: "muted" | "primary" | "subtle" = "primary",
): SdkworkGenerationStyle {
  if (tone === "muted") {
    return {
      color: "color-mix(in srgb, white 72%, var(--sdk-color-brand-accent))",
    };
  }

  if (tone === "subtle") {
    return {
      color: "color-mix(in srgb, white 64%, var(--sdk-color-brand-accent))",
    };
  }

  return {
    color: "color-mix(in srgb, white 92%, var(--sdk-color-brand-accent))",
  };
}
