// How a menu's popup comes and goes: transitions.dev's "Dropdown menu morph"
// (Jakub Antalik), ported to a `Menu`'s popup states. On the web the trigger
// grows into the panel; an NSH popup is its own surface beside the trigger,
// so it grows out of the trigger instead: from the corner nearest it
// (transform-origin), slid toward it, a touch small and blurred, fading in,
// on the morph's springy open curve; it leaves the same way on a quicker,
// unsprung curve. With reduced motion it appears and goes at once.
//
// Use on a `Menu`:
//   popupStyle={menuPopupStyle("top", "end")}
//   popupStartingStyle={menuPopupHidden("top", "end")}
//   popupEndingStyle={menuPopupLeaving("top", "end")}

import { reduceMotion } from "nsh";
import type { Align, Placement } from "nsh/base";
import type { ViewStyle } from "nsh";
import { DISTANCE, SPRING, springCss } from "./tokens";

/** The morph's own spring curves: opening settles with a light physical
 * pop; closing leaves swiftly without bounce. */
export const MENU_EASE = {
  open: springCss(SPRING.pop),
  close: springCss(SPRING.exit),
} as const;

/** How small the popup starts (`--morph-scale`) and how blurred
 * (`--morph-blur`, px). */
const START_SCALE = 0.97;
const START_BLUR = 2;

const MOVING = "opacity, transform, filter";

/** The popup at rest: how it moves into place on a physical spring that
 * times itself until settled. */
export function menuPopupStyle(placement: Placement, align: Align = "start"): ViewStyle {
  if (reduceMotion()) return { transitionProperty: "none" };
  return {
    transformOrigin: origin(placement, align),
    transitionProperty: MOVING,
    transitionTimingFunction: MENU_EASE.open,
  };
}

/** The corner of the popup nearest its trigger, which it grows from. */
function origin(placement: Placement, align: Align): string {
  const edge = { top: "bottom", bottom: "top", left: "right", right: "left" }[placement];
  const vertical = placement === "top" || placement === "bottom";
  const along =
    align === "center" ? "center" : vertical ? (align === "start" ? "left" : "right") : align === "start" ? "top" : "bottom";
  return vertical ? `${along} ${edge}` : `${edge} ${along}`;
}

/** Slid toward the trigger (`--morph-slide`, here `DISTANCE.base`: a
 * menu's panel travels less than the morph's plus button). */
function hidden(placement: Placement, align: Align): ViewStyle {
  const d = DISTANCE.base;
  const slide = {
    top: `translateY(${d}px)`,
    bottom: `translateY(${-d}px)`,
    left: `translateX(${d}px)`,
    right: `translateX(${-d}px)`,
  }[placement];
  return {
    opacity: 0,
    transform: `${slide} scale(${START_SCALE})`,
    transformOrigin: origin(placement, align),
    filter: `blur(${START_BLUR}px)`,
  };
}

/** The popup's first frame: where it opens from. */
export function menuPopupHidden(placement: Placement, align: Align = "start"): ViewStyle {
  if (reduceMotion()) return {};
  return hidden(placement, align);
}

/** The popup while it goes: back the way it came on a crisp exit spring,
 * without overshoot. */
export function menuPopupLeaving(placement: Placement, align: Align = "start"): ViewStyle {
  if (reduceMotion()) return { opacity: 0 };
  return {
    ...hidden(placement, align),
    transitionTimingFunction: MENU_EASE.close,
  };
}
