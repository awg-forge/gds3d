import type { ColorThemeId, ThemeColors } from "@models/preferences";

export type ColorPlan = "light" | "dark" | "lightAcrylic" | "darkAcrylic";
export type { ColorThemeId } from "@models/preferences";

export type { ThemeColors } from "@models/preferences";

export interface ThemeDefinition {
  id: ColorThemeId;
  name: string;
  description: string;
  author: string;
  version: string;
  light: ThemeColors;
  dark: ThemeColors;
  lightAcrylic: ThemeColors;
  darkAcrylic: ThemeColors;
}
