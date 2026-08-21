import { get, writable } from "svelte/store";
import * as preferencesApi from "@api/settings";
import { setLocale } from "@i18n";
import {
  SystemTheme,
  type ApplicationSettingsUpdate,
  type ConnectionSettingsUpdate,
  type LightweightSettingsUpdate,
  type Preferences,
  type ThemePreference,
} from "@models/preferences";
import { applyColorTheme } from "@themes/apply";
import { DEFAULT_CUSTOM_THEME } from "@themes/custom";
import { applyTypography, DEFAULT_FONT_SIZE } from "@themes/typography";
import { getP2pStatus, onP2pStatus } from "@api/p2p";
import { emptyP2pStatus, type P2pStatus } from "@models/p2p";
import type { IncomingInvite } from "@domain/invitations";
import { mergePreferences } from "./state-utils";
import { toast } from "svelte-sonner";

export const defaults: Preferences = {
  theme: "system",
  colorTheme: "default",
  customTheme: DEFAULT_CUSTOM_THEME,
  fontSize: DEFAULT_FONT_SIZE,
  fontFamily: "",
  splashDurationMs: 1000,
  silentStart: false,
  autoUpdate: true,
  locale: "zh-CN",
  rememberWindowState: true,
  windowMaterial: "solid",
  autoLightweightMinutes: null,
  hostUriLifetime: "always",
  joinUri: "",
  joinPort: 25565,
  reconnectTimeoutSecs: null,
  relayCustom: false,
  relayUrl: "",
  backgroundEnabled: false,
  backgroundImage: "",
  backgroundOpacity: 0.75,
  backgroundBlur: 0,
  backgroundBrightness: 1,
  backgroundCardBlur: 8,
};

export type SectionId =
  | "create"
  | "join"
  | "openfrp"
  | "sakurafrp"
  | "toolbox"
  | "personalize"
  | "settings"
  | "about";
export const activeSection = writable<SectionId>(loadSection());
export const sidebarCollapsed = writable(false);
export const preferences = writable<Preferences>(structuredClone(defaults));
export const session = writable<P2pStatus>({ ...emptyP2pStatus });
export const incomingInvite = writable<IncomingInvite | null>(null);

export type ToastTone = "success" | "error" | "info";
let unlisten: (() => void) | null = null;
let preferencesLoaded = false;
let systemTheme = SystemTheme.Light;
let nextInviteId = 0;

function loadSection(): SectionId {
  const value = localStorage.getItem("sealantern.active-section");
  const sections: SectionId[] = [
    "create",
    "join",
    "openfrp",
    "sakurafrp",
    "toolbox",
    "personalize",
    "settings",
    "about",
  ];
  return sections.includes(value as SectionId) ? (value as SectionId) : "create";
}

export function navigate(section: SectionId): void {
  activeSection.set(section);
  localStorage.setItem("sealantern.active-section", section);
}

export function toggleSidebar(): void {
  sidebarCollapsed.update((value) => !value);
}

export function importInvite(uri: string): void {
  incomingInvite.set({ id: ++nextInviteId, uri });
  navigate("join");
}

export function consumeInvite(id: number): void {
  incomingInvite.update((value) => (value?.id === id ? null : value));
}

export function showToast(message: string, tone: ToastTone = "info"): void {
  const options = { duration: tone === "error" ? 4000 : 2600 };
  if (tone === "success") toast.success(message, options);
  else if (tone === "error") toast.error(message, options);
  else toast.info(message, options);
}

function applyPreferences(value: Preferences): void {
  applyColorTheme(
    value.theme,
    value.colorTheme,
    systemTheme === SystemTheme.Dark,
    value.windowMaterial,
    value.customTheme,
  );
  applyTypography(value.fontSize, value.fontFamily);
}

export async function loadPreferences(): Promise<void> {
  if (preferencesLoaded) {
    applyPreferences(get(preferences));
    return;
  }
  try {
    const value = await preferencesApi.getPreferences();
    systemTheme = await preferencesApi
      .getSystemTheme()
      .catch(() =>
        window.matchMedia("(prefers-color-scheme: dark)").matches
          ? SystemTheme.Dark
          : SystemTheme.Light,
      );
    preferences.set(value);
    setLocale(value.locale);
  } catch (error) {
    console.error("Failed to load preferences", error);
  }
  preferencesLoaded = true;
  applyPreferences(get(preferences));
}

export function setTheme(theme: ThemePreference): void {
  preferences.update((value) => ({ ...value, theme }));
  const value = get(preferences);
  applyPreferences(value);
  void preferencesApi
    .saveTheme(theme, systemTheme)
    .then(() => applyPreferences(get(preferences)))
    .catch((error) => console.error("Failed to save theme", error));
}

export function getEffectiveTheme(theme: ThemePreference): "light" | "dark" {
  if (theme !== "system") return theme;
  return systemTheme === SystemTheme.Dark ? "dark" : "light";
}

export function startSystemThemeListener(): () => void {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const handleChange = (): void => {
    const value = get(preferences);
    if (value.theme !== "system") return;
    systemTheme = window.matchMedia("(prefers-color-scheme: dark)").matches
      ? SystemTheme.Dark
      : SystemTheme.Light;
    applyPreferences(value);
    void preferencesApi
      .saveTheme("system", systemTheme)
      .catch((error) => console.error("Failed to sync system theme", error));
  };
  media.addEventListener("change", handleChange);
  return () => media.removeEventListener("change", handleChange);
}

export function changeLocale(value: Preferences["locale"]): void {
  preferences.update((current) => ({ ...current, locale: value }));
  setLocale(value);
  void preferencesApi
    .saveLocale(value)
    .catch((error) => console.error("Failed to save locale", error));
}

export function updatePreferences(update: Partial<Preferences>): void {
  preferences.update((value) => mergePreferences(value, update));
  const value = get(preferences);
  void preferencesApi
    .savePersonalization(
      {
        theme: value.theme,
        colorTheme: value.colorTheme,
        customTheme: value.customTheme,
        fontSize: value.fontSize,
        fontFamily: value.fontFamily,
        windowMaterial: value.windowMaterial,
        backgroundEnabled: value.backgroundEnabled,
        backgroundImage: value.backgroundImage,
        backgroundOpacity: value.backgroundOpacity,
        backgroundBlur: value.backgroundBlur,
        backgroundBrightness: value.backgroundBrightness,
        backgroundCardBlur: value.backgroundCardBlur,
      },
      systemTheme,
    )
    .then(() => applyPreferences(get(preferences)))
    .catch((error) => console.error("Failed to save personalization", error));
}

export function updateConnection(update: ConnectionSettingsUpdate): void {
  preferences.update((value) => ({ ...value, ...update }));
  void preferencesApi
    .saveConnectionSettings(update)
    .catch((error) => console.error("Failed to save connection settings", error));
}

export function updateApplication(update: ApplicationSettingsUpdate): void {
  preferences.update((value) => ({ ...value, ...update }));
  void preferencesApi
    .saveApplicationSettings(update)
    .catch((error) => console.error("Failed to save application settings", error));
}

export function updateLightweight(update: LightweightSettingsUpdate): void {
  preferences.update((value) => ({ ...value, ...update }));
  void preferencesApi
    .saveLightweightSettings(update)
    .catch((error) => console.error("Failed to save lightweight settings", error));
}

export async function initializeSession(): Promise<void> {
  session.set(await getP2pStatus());
  unlisten?.();
  unlisten = await onP2pStatus((value) => session.set(value));
}

export function disposeSession(): void {
  unlisten?.();
  unlisten = null;
}
