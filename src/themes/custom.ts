import { getThemeColors } from ".";
import type { CustomTheme } from "@models/preferences";

export const DEFAULT_CUSTOM_THEME: CustomTheme = {
  light: { ...getThemeColors("default", "light") },
  dark: { ...getThemeColors("default", "dark") },
};
