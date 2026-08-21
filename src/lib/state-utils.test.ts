import { describe, expect, it } from "vitest";
import type { Preferences } from "@models/preferences";
import { mergePreferences } from "./state-utils";

const defaults = {
  theme: "system",
  customTheme: {
    light: { bg: "#ffffff" },
    dark: { bg: "#000000" },
  },
} as Preferences;

describe("mergePreferences", () => {
  it("applies a partial update without changing unrelated settings", () => {
    const result = mergePreferences(defaults, { fontSize: 18, locale: "en" });

    expect(result.fontSize).toBe(18);
    expect(result.locale).toBe("en");
    expect(result.theme).toBe(defaults.theme);
  });

  it("clones nested updates instead of retaining caller-owned references", () => {
    const customTheme = structuredClone(defaults.customTheme);
    const result = mergePreferences(defaults, { customTheme });

    customTheme.light.bg = "#000000";
    expect(result.customTheme.light.bg).toBe(defaults.customTheme.light.bg);
    expect(result.customTheme).not.toBe(defaults.customTheme);
  });
});
