/**
 * App-level i18n. Lightweight: no library, just a translations map + a
 * reactive `t()` function driven by the settings store's `language` field.
 */

import { computed, watchEffect } from 'vue';
import { useSettingsStore } from '../stores/settings';
import { en } from './en';
import { zh } from './zh';

export { en, zh };

export const dicts = { en, zh } as const;
export type Lang = keyof typeof dicts;

/**
 * Detect system language, defaulting between 'zh' and 'en'.
 */
export function detectSystemLanguage(): Lang {
  try {
    const nav = typeof navigator !== 'undefined' ? navigator.language || '' : '';
    if (/^zh/i.test(nav)) return 'zh';
  } catch {}
  return 'en';
}

/** The raw dictionaries, for features that search translations (#352 settings
 *  search looks an English keyword up in `en` and matches the translation). */
export function getDict(lang: string): unknown {
  return dicts[lang as Lang] || en;
}

export function useI18n() {
  const settings = useSettingsStore();
  const dict = computed(() => dicts[settings.language as Lang] || (detectSystemLanguage() === 'zh' ? zh : en));

  function lookup(d: any, parts: string[]): string | undefined {
    let cur: any = d;
    for (const p of parts) {
      if (cur == null) return undefined;
      cur = cur[p];
    }
    return typeof cur === 'string' ? cur : undefined;
  }

  function t(key: string, params?: Record<string, string | number>): string {
    const parts = key.split('.');
    let str = lookup(dict.value, parts) ?? lookup(en, parts) ?? key;
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        str = str.replace(new RegExp(`\\{${k}\\}`, 'g'), String(v));
      }
    }
    return str;
  }

  return { t, lang: computed(() => settings.language) };
}

/** BCP 47 tag for `<html lang>`; Chinese uses Simplified region. */
export function htmlLangFor(lang: string): string {
  const resolved = lang === 'auto' ? detectSystemLanguage() : lang;
  return resolved === 'zh' ? 'zh-CN' : 'en';
}

/**
 * Keep `<html lang>` on the UI language. Called once per window (main.ts)
 * after Pinia is installed.
 */
export function followHtmlLang(): void {
  const settings = useSettingsStore();
  watchEffect(() => {
    document.documentElement.lang = htmlLangFor(settings.language);
  });
}
