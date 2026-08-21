import { get, writable } from "svelte/store";
import { createSubscriber } from "svelte/reactivity";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import type { Locale } from "@models/preferences";
import type { P2pErrorCode } from "@models/p2p";

type TranslationNode = string | { [key: string]: TranslationNode };

const translations: Record<Locale, TranslationNode> = {
  "zh-CN": zhCN,
  en,
};

export const locale = writable<Locale>("zh-CN");
const subscribeToLocale = createSubscriber((update) => locale.subscribe(update));

export function setLocale(value: Locale) {
  locale.set(value);
  document.documentElement.lang = value;
}

function resolve(node: TranslationNode, key: string): string | undefined {
  let current: TranslationNode | undefined = node;
  for (const segment of key.split(".")) {
    if (typeof current === "string") return undefined;
    current = current[segment];
    if (current === undefined) return undefined;
  }
  return typeof current === "string" ? current : undefined;
}

export function t(key: string, params: Record<string, string | number> = {}) {
  subscribeToLocale();
  const currentLocale = get(locale);
  let value = resolve(translations[currentLocale], key) ?? resolve(translations.en, key) ?? key;
  for (const [name, replacement] of Object.entries(params)) {
    value = value.split(`{${name}}`).join(String(replacement));
  }
  return value;
}

/** Converts backend messages into locale-aware copy before they reach the UI. */
export function backendMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  const portMatch = message.match(
    /^no Minecraft world is available on port (\d+); make sure the world is open to LAN$/,
  );
  if (portMatch) return t("errors.minecraftUnavailable", { port: portMatch[1] });

  const reconnectMatch = message.match(/^restoring P2P link, attempt (\d+)$/);
  if (reconnectMatch) return t("connection.reconnecting", { attempt: reconnectMatch[1] });

  const playerJoinedMatch = message.match(/^player (.+) joined$/);
  if (playerJoinedMatch) return t("connection.playerJoined", { player: playerJoinedMatch[1] });

  const playerLeftMatch = message.match(/^player (.+) left$/);
  if (playerLeftMatch) return t("connection.playerLeft", { player: playerLeftMatch[1] });

  if (message.startsWith("P2P link closed: ")) return t("connection.disconnected");

  const pathMatch = message.match(/^using a (relay|direct) P2P path with (\d+) ms latency$/);
  if (pathMatch) {
    return t("connection.pathChanged", {
      route: pathMatch[1] === "relay" ? t("join.relay") : t("join.direct"),
      latency: pathMatch[2],
    });
  }

  const knownMessages: Record<string, string> = {
    "stop the current P2P session first": "errors.connectionOccupied",
    "Minecraft port must be between 1 and 65535": "errors.minecraftPortRange",
    "local port must be between 1 and 65535": "errors.localPortRange",
    "invalid room invitation lifetime": "errors.invalidInviteLifetime",
    "the Minecraft world was closed, so the room stopped automatically":
      "errors.minecraftWorldClosed",
    "node startup timed out; check relay settings": "errors.nodeStartupTimedOut",
    "P2P link established": "connection.connectedToHost",
    "P2P link restored": "connection.restored",
  };
  const category = message.replace(/^Error: /, "");
  const errorCategory = `errors.category.${category}`;
  if (resolve(translations.en, errorCategory)) return t(errorCategory);
  if (knownMessages[message]) return t(knownMessages[message]);

  return t("errors.unexpected");
}

export function p2pErrorMessage(code: P2pErrorCode): string {
  return t(`errors.category.${code}`);
}
