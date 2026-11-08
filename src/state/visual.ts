// Which music light plays (the player bar's and Now Playing's, the same
// one), remembered between runs.

import { signal, Storage } from "nsh";
import { EFFECT_NAMES, TOOLBAR_EFFECTS, type ToolbarEffect } from "../components/ui/toolbar-fx";

const KEY = "visual:effect";
const chosen = signal<ToolbarEffect>("dots");

Storage.getItem(KEY)
  .then((saved) => {
    if (saved && (TOOLBAR_EFFECTS as string[]).includes(saved)) chosen.set(saved as ToolbarEffect);
  })
  .catch(() => {});

/** The music light in use (tracked). */
export const lightEffect = () => chosen();

/** Its name, for the switch's label (tracked). */
export const lightName = () => EFFECT_NAMES[chosen()];

/** Uses `effect` from now on, and remembers it. */
export function setLight(effect: ToolbarEffect): void {
  chosen.set(effect);
  Storage.setItem(KEY, effect).catch(() => {});
}
