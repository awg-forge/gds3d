import { get, writable } from "svelte/store";
import { createSubscriber } from "svelte/reactivity";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
export type Locale = "zh-CN" | "en";

type TranslationNode = string | { [key: string]: TranslationNode };

const translations: Record<Locale, TranslationNode> = {
  "zh-CN": zhCN,
  en,
};

export const locale = writable<Locale>("en");
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
