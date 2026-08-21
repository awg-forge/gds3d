import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";

export type AppUpdate = Update;

export function getAppVersion(): Promise<string> {
  return getVersion();
}

export function openProjectRepository(): Promise<void> {
  return openUrl("https://github.com/SeaLantern-Studio/SeaLantern-Connect");
}

export function checkForAppUpdate(): Promise<AppUpdate | null> {
  return check();
}

export async function installAppUpdate(update: AppUpdate): Promise<void> {
  await update.downloadAndInstall();
  await relaunch();
}

export function markFrontendReady(): Promise<void> {
  return invoke("frontend_ready");
}

export function markPageLoaded(page: string): Promise<void> {
  return invoke("frontend_page_loaded", { page });
}
