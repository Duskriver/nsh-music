// App preferences persisted in Storage: language locale, appearance scheme
// (when no Omarchy system theme overrides it), and desktop close-to-tray behavior.

import { setColorScheme, signal, Storage } from "nsh";
import { setLocale } from "../i18n";
import { usingSystemTheme } from "../platform/theme";

export type AppearancePreference = "system" | "light" | "dark";
export type LanguagePreference = "system" | "zh-CN" | "en";

const APPEARANCE_KEY = "ui:appearance";
const LOCALE_KEY = "ui:locale";
const HIDE_ON_CLOSE_KEY = "desktop:hideOnClose";
const VALID_APPEARANCE: readonly AppearancePreference[] = ["system", "light", "dark"];
const VALID_LOCALE: readonly LanguagePreference[] = ["system", "zh-CN", "en"];

const appearance = signal<AppearancePreference>("system");
const language = signal<LanguagePreference>("system");
const hideClose = signal(true);

Storage.getItem(APPEARANCE_KEY)
  .then((saved) => {
    if (saved && (VALID_APPEARANCE as readonly string[]).includes(saved)) {
      const pref = saved as AppearancePreference;
      appearance.set(pref);
      if (!usingSystemTheme()) setColorScheme(pref);
    }
  })
  .catch(() => {});

Storage.getItem(LOCALE_KEY)
  .then((saved) => {
    if (saved && (VALID_LOCALE as readonly string[]).includes(saved)) {
      const pref = saved as LanguagePreference;
      language.set(pref);
      setLocale(pref === "system" ? null : pref);
    }
  })
  .catch(() => {});

Storage.getItem(HIDE_ON_CLOSE_KEY)
  .then((saved) => {
    if (saved === "0" || saved === "1") hideClose.set(saved !== "0");
  })
  .catch(() => {});

/** The user's chosen language preference (tracked). */
export const languagePreference = () => language();

/** Updates and persists the UI language preference, switching `nsh/i18n` immediately. */
export function setLanguagePreference(pref: LanguagePreference): void {
  language.set(pref);
  Storage.setItem(LOCALE_KEY, pref).catch(() => {});
  setLocale(pref === "system" ? null : pref);
}

/** The user's chosen appearance preference (tracked). */
export const appearancePreference = () => appearance();

/** Updates and persists the appearance preference, applying it immediately unless controlled by the system theme. */
export function setAppearancePreference(pref: AppearancePreference): void {
  appearance.set(pref);
  Storage.setItem(APPEARANCE_KEY, pref).catch(() => {});
  if (!usingSystemTheme()) setColorScheme(pref);
}

/** Whether closing the main desktop window hides it to the tray instead of quitting (tracked). */
export const hideOnClose = () => hideClose();

/** Updates and persists whether closing the main desktop window hides to the tray. */
export function setHideOnClose(on: boolean): void {
  hideClose.set(on);
  Storage.setItem(HIDE_ON_CLOSE_KEY, on ? "1" : "0").catch(() => {});
}
