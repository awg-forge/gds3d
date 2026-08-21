import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type {
  ApplicationSettingsUpdate,
  ConnectionSettingsUpdate,
  HostUriLifetime,
  Locale,
  LightweightSettingsUpdate,
  PersonalizationUpdate,
  Preferences,
  SystemTheme,
  ThemePreference,
} from "@models/preferences";

export function getPreferences(): Promise<Preferences> {
  return invoke("get_preferences");
}

export function getSystemFonts(): Promise<string[]> {
  systemFontsPromise ??= invoke("get_system_fonts");
  return systemFontsPromise;
}

let systemFontsPromise: Promise<string[]> | null = null;

export function getSystemTheme(): Promise<SystemTheme> {
  return invoke("get_system_theme");
}

export function supportsLiquidGlass(): Promise<boolean> {
  return invoke("supports_liquid_glass");
}

export async function saveTextFile(content: string, defaultPath: string): Promise<boolean | null> {
  const path = await save({
    defaultPath,
    filters: [{ name: "CSS", extensions: ["css"] }],
  });
  if (!path) return null;
  await invoke("write_text_file", { path, content });
  return true;
}

export async function openTextFile(): Promise<string | null> {
  const selected = await open({
    multiple: false,
    filters: [{ name: "CSS", extensions: ["css"] }],
  });
  const path = Array.isArray(selected) ? selected[0] : selected;
  return path ? invoke<string>("read_text_file", { path }) : null;
}

export function saveTheme(theme: ThemePreference, systemTheme: SystemTheme): Promise<void> {
  return invoke("set_theme", { theme, systemTheme });
}

export function saveLocale(locale: Locale): Promise<void> {
  return invoke("set_locale", { locale });
}

export function saveInviteLifetime(lifetime: HostUriLifetime): Promise<void> {
  return invoke("set_invite_lifetime", { lifetime });
}

export function savePersonalization(
  update: PersonalizationUpdate,
  systemTheme: SystemTheme,
): Promise<void> {
  return invoke("set_personalization", { update, systemTheme });
}

export function saveApplicationSettings(update: ApplicationSettingsUpdate): Promise<void> {
  return invoke("set_application_settings", { update });
}

export function saveConnectionSettings(update: ConnectionSettingsUpdate): Promise<void> {
  return invoke("set_connection_settings", { update });
}

export function saveLightweightSettings(update: LightweightSettingsUpdate): Promise<void> {
  return invoke("set_lightweight_settings", { update });
}
