import defaultTheme from "./default";
import inkstoneTheme from "./inkstone";
import vellumTheme from "./vellum";
import mossTheme from "./moss";
import gloamingTheme from "./gloaming";
import type { ColorPlan, ColorThemeId, ThemeColors, ThemeDefinition } from "./types";

type PresetColorThemeId = Exclude<ColorThemeId, "custom">;

const themes: Record<PresetColorThemeId, ThemeDefinition> = {
  default: defaultTheme,
  inkstone: inkstoneTheme,
  vellum: vellumTheme,
  moss: mossTheme,
  gloaming: gloamingTheme,
};

export function getThemeOptions(): Array<{ label: string; value: ColorThemeId }> {
  return [
    ...Object.values(themes).map((theme) => ({ label: theme.name, value: theme.id })),
    { label: "Custom", value: "custom" },
  ];
}

export function getThemeColors(themeId: ColorThemeId, plan: ColorPlan): ThemeColors {
  const preset = themeId === "custom" ? themes.default : themes[themeId];
  return (preset ?? themes.default)[plan];
}

export type { ColorPlan, ColorThemeId, ThemeColors, ThemeDefinition } from "./types";
