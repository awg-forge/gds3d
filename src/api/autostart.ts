import {
  disable as disablePlugin,
  enable as enablePlugin,
  isEnabled,
} from "@tauri-apps/plugin-autostart";

export function getAutostartEnabled(): Promise<boolean> {
  return isEnabled();
}

export function enableAutostart(): Promise<void> {
  return enablePlugin();
}

export function disableAutostart(): Promise<void> {
  return disablePlugin();
}
