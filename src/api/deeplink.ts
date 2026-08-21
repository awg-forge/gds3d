import { invoke } from "@tauri-apps/api/core";
import { getCurrent, onOpenUrl } from "@tauri-apps/plugin-deep-link";

export function onDeepLinks(callback: (urls: string[]) => void): Promise<() => void> {
  return onOpenUrl(callback);
}

export async function getInitialDeepLinks(): Promise<string[]> {
  const [current, pending] = await Promise.all([
    getCurrent(),
    invoke<string[]>("take_pending_links"),
  ]);
  return [...(pending ?? []), ...(current ?? [])];
}

export function getPendingDeepLinks(): Promise<string[]> {
  return invoke<string[]>("take_pending_links");
}
