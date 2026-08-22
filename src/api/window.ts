import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

const mainWindow = getCurrentWindow();

export function closeWindow(): Promise<void> {
  return mainWindow.close();
}

export function onExitRequested(callback: () => void): Promise<UnlistenFn> {
  return listen("gds3d-exit-requested", callback);
}

export function cancelExit(): Promise<void> {
  return invoke("cancel_exit");
}

export function confirmExit(): Promise<void> {
  return invoke("confirm_exit");
}

export function restartApplication(): Promise<void> {
  return invoke("restart_application");
}

export function markFrontendReady(): Promise<void> {
  return invoke("frontend_ready");
}

export function minimizeWindow(): Promise<void> {
  return mainWindow.minimize();
}

export function startWindowDragging(): Promise<void> {
  return mainWindow.startDragging();
}

export function toggleMaximize(): Promise<void> {
  return mainWindow.toggleMaximize();
}

export function isWindowMaximized(): Promise<boolean> {
  return mainWindow.isMaximized();
}

export function onWindowResized(callback: () => void): Promise<() => void> {
  return mainWindow.onResized(callback);
}
