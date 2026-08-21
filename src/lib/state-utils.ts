import type { Preferences } from "@models/preferences";

export function mergePreferences(current: Preferences, update: Partial<Preferences>): Preferences {
  return { ...current, ...structuredClone(update) };
}
