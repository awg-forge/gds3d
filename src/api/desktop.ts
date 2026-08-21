import { invoke } from "@tauri-apps/api/core";

export interface DesktopPreferences {
  rememberWindowState: boolean;
  closeToTray: boolean;
}

export function getDesktopPreferences(): Promise<DesktopPreferences> {
  return invoke("get_desktop_preferences");
}

export function getSystemFonts(): Promise<string[]> {
  return invoke("get_system_fonts");
}

export function updateDesktopPreferences(preferences: DesktopPreferences): Promise<void> {
  return invoke("update_desktop_preferences", { preferences });
}
