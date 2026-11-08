// The app's styled controls, each wrapping a base component once. Pages and
// app components compose these; they do not restyle base controls.
//
// In a terminal (`terminal`), images do not draw: icon-only controls show a
// short text label, and covers take no Cells.

import { type AnimatedValue, Canvas, effect, Image, platform, type MotionSpec, reduceMotion, Text, token, untracked, View } from "nsh";
import type { CanvasRenderingContext2D, JSX } from "nsh";
import { Button, Slider, Tooltip } from "nsh/base";
import { alpha, tint } from "../../state/tint";
import { imageFor } from "../../data/images";
import { Icon, type IconColor, iconMotion, type IconName, SwapIcon } from "./icons";
import { lazyMotion } from "./motion";
import { DISTANCE, DURATION, DURATION_CLASS, EASE_CLASS, SCALE, SMOOTH_OUT } from "./tokens";

/** The app runs in a terminal: Cells and text, no images. */
export const terminal = platform.idiom === "terminal";

/** The focus ring every control shares. */
export const focusRing = "focus:outline focus:outline-2 focus:outline-ring";

/** The popup surface every menu, popover and tooltip-like panel shares. */
export const popupSurface = "rounded-(--radius-card) border border-border bg-popover p-1 shadow-lg";

/** A menu row: compact, the same padding, radius and states everywhere. */
export const menuItem =
  "h-8 flex-row items-center rounded-(--radius) px-2.5 hover:bg-muted focus:bg-muted active:bg-muted disabled:opacity-40";

/** How an animated layer of a control moves between its states (rest,
 * hover, press): quick, eased, still with reduced motion. */
export const LAYER = `transition ${DURATION_CLASS.fast} ${EASE_CLASS.out} motion-reduce:transition-none`;

/** How a control, or a layer of it, presses in and settles back (after
 * affectui's clean button: a short, soft ease-out; still with reduced
 * motion). */
const PRESS = `transition ${DURATION_CLASS.fast} ${EASE_CLASS.smoothOut} motion-reduce:transition-none`;

/** How a wash or a colour comes and goes: quicker than a press. */
const WASH = `transition ${DURATION_CLASS.instant} ${EASE_CLASS.out} motion-reduce:transition-none`;

/** A whole button pressed: a little smaller, not on reduced motion. */
const PRESSED = `active:scale-[${SCALE.pressSoft}] disabled:active:scale-100 motion-reduce:active:scale-100`;

/** A label coming in after it changed (affectui's label morph, without its
 * blur): up a little from below as it fades in. */
const LABEL_IN: MotionSpec = {
  keyframes: { opacity: [0, 1], translateY: [DISTANCE.small, 0] },
  duration: DURATION.slow,
  easing: SMOOTH_OUT,
};

/** A hint bubble: shown by the desktops while the pointer rests on a
 * control, and as the keyboard focus reaches it. */
export function Hint(props: { label: string }) {
  return (
    <Tooltip class="rounded-(--radius) bg-popover px-2 py-1 shadow-md" placement="right">
      <Text class="text-xs font-medium text-popover-foreground">{props.label}</Text>
    </Tooltip>
  );
}

export interface IconButtonProps {
  name: IconName;
  /** What it does: screen readers say it, a terminal shows it. */
  label: string;
  /** A terminal's shorter label (default `label`). */
  short?: string;
  /** Points (default 18). */
  size?: number;
  /** A toggle that is on: drawn in the primary color, with a dot. */
  pressed?: boolean;
  /** The color while `pressed`, instead of the primary color and the dot
   * (Liked's red heart). */
  pressedColor?: IconColor;
  /** While `pressed`, the colour of what plays (`state/tint`), not the
   * primary. */
  tinted?: boolean;
  disabled?: boolean;
  /** Shows `label` in a tooltip (for icons whose meaning is not obvious). */
  hint?: boolean;
  /** Selected (a navigation destination): screen readers say so. */
  selected?: boolean;
  onPress: () => void;
}

/** A borderless icon button: brighter on hover, primary while `pressed`. */
export function IconButton(props: IconButtonProps) {
  const size = () => props.size ?? 18;
  if (terminal) {
    return (
      <Button
        accessibilityLabel={props.label}
        disabled={props.disabled ?? false}
        selected={props.selected ?? false}
        onPress={props.onPress}
        class="px-1 focus:bg-muted disabled:opacity-50"
      >
        <Text class={props.pressed ? "font-bold text-primary" : "text-foreground"}>[{props.short ?? props.label}]</Text>
      </Button>
    );
  }
  const color = (): IconColor => (props.pressed ? (props.pressedColor ?? "primary") : "foreground");
  const hover = iconMotion();
  return (
    <View onMouseEnter={() => !props.disabled && hover.play(props.name)}>
    <Button
      accessibilityLabel={props.label}
      disabled={props.disabled ?? false}
      selected={props.selected ?? false}
      onPress={props.onPress}
      tooltip={props.hint ? <Hint label={props.label} /> : undefined}
      class={`group relative items-center justify-center rounded-full disabled:opacity-30 ${focusRing}`}
      style={{ width: size() + 16, height: size() + 16 }}
    >
      {/* Layer 1: a translucent disc that settles in under the pointer and
          deepens as it presses: whatever is behind shows through. */}
      <View class={`absolute inset-0 rounded-full bg-foreground/10 opacity-0 scale-[${SCALE.enter}] ${PRESS} group-hover:scale-100 group-hover:opacity-100 group-active:scale-[${SCALE.press}] group-active:bg-foreground/15 group-disabled:opacity-0`} />
      {/* Layer 2: the glyph brightens (its drawing morphs, `iconMotion`), then
          sinks as it presses; a new glyph swaps in (`SwapIcon`). */}
      <View
        class={`${PRESS} group-active:scale-[${SCALE.press}] group-disabled:scale-100 ${
          props.pressed ? "opacity-100" : "opacity-70 group-hover:opacity-100"
        }`}
      >
        <View style={hover.style}>
          <SwapIcon name={props.name} t={hover.t()} size={size()} color={color()} hex={props.pressed && props.tinted && !props.pressedColor ? alpha(tint(), 0.95) : undefined} />
        </View>
      </View>
      <If when={(props.pressed ?? false) && !props.pressedColor}>
        <View class="absolute bottom-0.5 size-1 rounded-full bg-primary" style={props.tinted ? { backgroundColor: alpha(tint(), 0.95) } : undefined} />
      </If>
    </Button>
    </View>
  );
}

/** The round play / pause button, `size` across. */
/** `quiet`: no border or shadow, a faint disc (Now Playing, where the music
 * light leads). */
export function PlayButton(props: { playing: boolean; size?: number; disabled?: boolean; tinted?: boolean; quiet?: boolean; onPress: () => void }) {
  const size = () => props.size ?? 36;
  const label = () => (props.playing ? "Pause" : "Play");
  if (terminal) {
    return (
      <Button accessibilityLabel={label()} disabled={props.disabled ?? false} onPress={props.onPress} class="px-1 focus:bg-muted disabled:opacity-50">
        <Text class="font-bold text-primary">[{label()}]</Text>
      </Button>
    );
  }
  const hover = iconMotion();
  return (
    <View onMouseEnter={() => !props.disabled && hover.play(props.playing ? "pause" : "play")}>
    <Button
      accessibilityLabel={label()}
      disabled={props.disabled ?? false}
      onPress={props.onPress}
      class={`group items-center justify-center overflow-hidden rounded-full ${props.quiet ? "" : "bg-background/80 shadow-md hover:shadow-lg active:shadow-sm"} ${PRESS} ${PRESSED} disabled:opacity-40 ${focusRing}`}
      style={{ width: size(), height: size(), ...(props.tinted || props.quiet ? { backgroundColor: alpha(tint(), props.quiet ? 0.08 : 0.16) } : {}) }}
    >
      {/* A shade that comes in on hover and deepens as it presses. */}
      <View class={`absolute inset-0 rounded-full bg-foreground opacity-0 ${WASH} group-hover:opacity-10 group-active:opacity-15 group-disabled:opacity-0`} />
      <View class={`${PRESS} group-active:scale-[${SCALE.press}] group-disabled:scale-100`}>
        <View style={hover.style}>
          <SwapIcon name={props.playing ? "pause" : "play"} t={hover.t()} size={size() * 0.4} color="foreground" hex={props.tinted ? tint() : undefined} />
        </View>
      </View>
    </Button>
    </View>
  );
}

/** A labeled button: `primary` for the one emphasized commitment of an
 * area, `outline` (default) for the others. */
export function ActionButton(props: {
  label: string;
  variant?: "primary" | "outline";
  disabled?: boolean;
  onPress: () => void;
}) {
  const primary = () => props.variant === "primary";
  if (terminal) {
    return (
      <Button disabled={props.disabled ?? false} onPress={props.onPress} class="px-1 focus:bg-muted disabled:opacity-50">
        <Text class={primary() ? "font-bold text-primary" : "text-foreground"}>[{props.label}]</Text>
      </Button>
    );
  }
  // A new label comes in (not the first: that one just shows). Made with
  // the first new label.
  const label = lazyMotion();
  let shown = untracked(() => props.label);
  effect(() => {
    const now = props.label;
    if (now === shown) return;
    shown = now;
    if (!untracked(reduceMotion)) void label.play(LABEL_IN);
  });
  return (
    <Button
      disabled={props.disabled ?? false}
      onPress={props.onPress}
      class={`group h-9 items-center justify-center overflow-hidden rounded-full px-5 ${PRESS} ${PRESSED} disabled:opacity-40 ${focusRing} ${
        primary() ? "bg-foreground shadow-sm hover:shadow-md" : "border border-border bg-secondary hover:border-muted-foreground"
      }`}
    >
      {/* A wash that comes in over the button on hover and deepens as it
          presses. */}
      <View
        class={`absolute inset-0 rounded-full opacity-0 ${WASH} group-hover:opacity-10 group-active:opacity-15 group-disabled:opacity-0 ${
          primary() ? "bg-background" : "bg-foreground"
        }`}
      />
      <Text class={`text-sm font-bold ${primary() ? "text-background" : "text-foreground"}`} style={label.style}>
        {props.label}
      </Text>
    </Button>
  );
}

/** A quiet text button for a link that leaves the app (the `link` role):
 * underlined on hover, pressed in as a whole. */
export function TextLink(props: { label: string; onPress: () => void; small?: boolean; class?: string }) {
  return (
    <Button
      role="link"
      onPress={props.onPress}
      class={`group self-start rounded-sm ${PRESS} ${PRESSED} active:opacity-70 ${focusRing} ${props.class ?? ""}`}
    >
      <Text class={`font-semibold text-primary hover:underline ${props.small ? "text-xs" : "text-sm"}`}>{props.label}</Text>
    </Button>
  );
}

/** An item's image, square (or round for an artist), with a glyph while it
 * has none. Sized by `size` (points) or by `class` (`w-full max-w-52`). In a
 * terminal it takes no Cells. `liked` is Liked Songs' own cover, a red heart
 * tile, in place of Spotify's artwork. */
export function Cover(props: {
  src: string;
  size?: number;
  round?: boolean;
  liked?: boolean;
  fallback?: IconName;
  class?: string;
}) {
  if (terminal) return <View class="hidden" />;
  const glyph = () => (props.size ? Math.max(16, props.size * 0.42) : 72);
  return (
    <View
      class={`shrink-0 items-center justify-center overflow-hidden ${props.liked ? "bg-liked" : "bg-secondary"} ${
        props.round ? "rounded-full" : "rounded-md"
      } ${props.size ? "" : "aspect-square w-full"} ${props.class ?? ""}`}
      style={props.size ? { width: props.size, height: props.size } : {}}
    >
      <Icon
        name={props.liked ? "heartFilled" : (props.fallback ?? "music")}
        size={props.liked ? glyph() : props.size ? Math.max(16, props.size * 0.4) : 40}
        color={props.liked ? "liked-foreground" : "muted-foreground"}
      />
      <If when={props.src && !props.liked}>
        <Image src={imageFor(props.src, props.size)} accessibilityElementsHidden class="absolute inset-0 object-cover" />
      </If>
    </View>
  );
}

/** A slider (position, volume): a track with a filled range and a thumb
 * that shows on hover, focus and drag. */
export function BarSlider(props: {
  label: string;
  value: number;
  max: number;
  disabled?: boolean;
  /** The range in the colour of what plays (`state/tint`). */
  tinted?: boolean;
  /** Thin and faint, for where something else leads (Now Playing). */
  quiet?: boolean;
  class?: string;
  onChange: (value: number) => void;
  onChangeEnd: (value: number) => void;
  /** Where the range ends, 0..1, as an animated value that moves natively
   * (the position while it plays): the range is drawn from it, every
   * display frame, with no JS, and `value` need only change when its
   * screen-reader value should. */
  fill?: AnimatedValue;
}) {
  const fill = props.fill;
  // The range drawn natively from `fill` (not in a terminal, which draws
  // its own slider of glyphs). The slider's own range stays for its
  // layout and states, transparent.
  const drawn = !!fill && !terminal;
  return (
    <Slider
      accessibilityLabel={props.label}
      value={props.value}
      min={0}
      max={props.max}
      step={0}
      disabled={props.disabled ?? false}
      class={props.class}
      trackClass="group h-5 justify-center"
      // The range fills the track, so both share one centre line.
      // One height at rest and on hover (the taller one): it never jumps.
      indicatorClass={`rounded-full ${props.quiet ? "h-1.5 bg-foreground/8" : "h-2 bg-foreground/12"}`}
      rangeClass={`h-full rounded-full ${props.quiet ? "bg-foreground/35" : "bg-foreground/55"} disabled:bg-muted-foreground ${drawn ? "opacity-0" : ""}`}
      // No thumb dot: the bar itself is the control (it still takes the
      // keyboard focus and screen readers through the slider).
      thumbClass="size-0 opacity-0"
      rangeStyle={
        props.tinted && !props.disabled && !drawn
          ? { backgroundImage: `linear-gradient(90deg, ${alpha(tint(), props.quiet ? 0.2 : 0.3)}, ${alpha(tint(), props.quiet ? 0.4 : 0.6)})` }
          : undefined
      }
      onChange={({ value }) => typeof value === "number" && props.onChange(value)}
      onChangeEnd={({ value }) => typeof value === "number" && props.onChangeEnd(value)}
    >
      {drawn ? (
        // Over the indicator (the track centres it), presses going through.
        <View class="pointer-events-none absolute inset-0 justify-center">
          <Canvas class={`w-full ${props.quiet ? "h-1.5" : "h-2"}`} draw={(cx, size) => drawRange(cx, size, fill, props)} />
        </View>
      ) : undefined}
    </Slider>
  );
}

/** The range of a `BarSlider` with `fill`, as its classes draw it: a pill
 * from the start to `fill` of the width (its corners rounded as
 * `rounded-full` rounds them, so a range shorter than it is tall is
 * narrower, not cut), in `bg-foreground/55` (`/35` quiet;
 * `muted-foreground` disabled) and, tinted, the cover's gradient over it,
 * spread over the range as `linear-gradient(90deg, ..)` is. Only the
 * colours run JS (when they change); the length is native. */
function drawRange(
  cx: CanvasRenderingContext2D,
  size: { width: number; height: number },
  fill: AnimatedValue,
  props: { quiet?: boolean; disabled?: boolean; tinted?: boolean },
) {
  const { width: w, height: h } = size;
  if (w <= 0 || h <= 0) return;
  const length = fill.mul(w);
  // `rounded-full`: half the shorter side.
  const radius = length.interpolate({ inputRange: [0, h], outputRange: [0, h / 2] });
  cx.beginPath();
  cx.roundRect(0, 0, length, h, radius);
  cx.clip();
  cx.fillStyle = props.disabled ? String(token("muted-foreground")) : alpha(String(token("foreground")), props.quiet ? 0.35 : 0.55);
  cx.fillRect(0, 0, w, h);
  if (props.tinted && !props.disabled) {
    // Drawn at full width and squeezed to the range's: the gradient spans
    // the range, whatever its length.
    const gradient = cx.createLinearGradient(0, 0, w, 0);
    gradient.addColorStop(0, alpha(tint(), props.quiet ? 0.2 : 0.3));
    gradient.addColorStop(1, alpha(tint(), props.quiet ? 0.4 : 0.6));
    cx.scale(fill, 1);
    cx.fillStyle = gradient;
    cx.fillRect(0, 0, w, h);
  }
}

/** A section's title. */
export function SectionTitle(props: { children: JSX.Element; class?: string }) {
  return (
    <Text role="header" class={`text-xl font-bold text-foreground ${props.class ?? ""}`}>
      {props.children}
    </Text>
  );
}
