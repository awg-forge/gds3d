import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";

export function getAppVersion(): Promise<string> {
  return getVersion();
}

export function openManual(): Promise<void> {
  return invoke("open_manual");
}
