// Strongly typed wrapper over `nsh/i18n`.
// Translation keys, required `%{param}` arguments, and locales are generated
// from `locales/*` by `node scripts/i18n.mjs generate`.

import {
  addTranslations,
  availableLocales,
  setLocale,
  t as rawT,
} from "nsh/i18n";
import type {
  AppLocale,
  NoParamTranslationKey,
  TranslationKey,
  TranslationParams,
} from "./translations.generated";

export { addTranslations, availableLocales, setLocale };
export type { AppLocale, NoParamTranslationKey, TranslationKey, TranslationParams };

type LocaleOption = { locale?: AppLocale | (string & {}) };

type ArgsFor<K extends TranslationKey> =
  keyof TranslationParams[K] extends never
    ? [options?: LocaleOption]
    : [options: TranslationParams[K] & LocaleOption];

/** Type-safe translation lookup with compile-time validation of keys and `%{param}` arguments. */
export function t<K extends TranslationKey>(key: K, ...args: ArgsFor<K>): string {
  return rawT(key, args[0]);
}

/** Explicit escape hatch for dynamic translation keys loaded at run time via `addTranslations`. */
export function tDynamic(key: string, options?: { locale?: string; [name: string]: unknown }): string {
  return rawT(key, options);
}
