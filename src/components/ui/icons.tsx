// The app's icons: one family of glyphs (Tabler Icons, from nsh-icon's
// `nsh-icon-tabler`), drawn by nsh-icon's `Icon` in a theme token's text
// color. A hover morph redraws a glyph from its SVG. A terminal draws no
// images: icon-only controls show text there instead (see `controls.tsx`).

import { batch, cleanup, colorScheme, effect, type MotionName, type MotionSpec, reduceMotion, signal, type Tokens, token, untracked, View } from "nsh";
import { Icon as Glyph } from "nsh-icon";
import {
  Activity,
  AlertCircle,
  ArrowNarrowDown,
  ArrowNarrowUp,
  ArrowsShuffle,
  ArrowUpRight,
  Books,
  BrandGithub,
  Check,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  Clock,
  Code,
  Compass,
  Disc,
  Heart,
  HeartFilled,
  Home,
  InfoCircle,
  Keyboard,
  LayoutSidebarLeftCollapse,
  LayoutSidebarLeftExpand,
  Login,
  Logout,
  Microphone,
  Music,
  PlayerPauseFilled,
  PlayerPlayFilled,
  PlayerSkipBackFilled,
  PlayerSkipForwardFilled,
  Power,
  Repeat,
  RepeatOnce,
  Search,
  Settings,
  Sparkles,
  User,
  Volume,
  Volume2,
  Volume3,
  X,
} from "nsh-icon-tabler";
import { lazyMotion } from "./motion";
import { BLUR, DURATION, EASING, SCALE } from "./tokens";

export type IconName =
  | "home" | "search" | "library" | "play" | "pause" | "skipBack" | "skipForward"
  | "shuffle" | "repeat" | "repeatOne" | "heart" | "heartFilled" | "volume" | "volumeLow"
  | "volumeMute" | "chevronLeft" | "chevronRight" | "chevronDown" | "music" | "user"
  | "logIn" | "logOut" | "disc" | "clock" | "alert" | "close"
  | "sparkles" | "externalLink" | "info" | "github" | "code" | "check"
  | "sidebarCollapse" | "sidebarExpand" | "power" | "activity" | "compass" | "microphone"
  | "arrowUp" | "arrowDown" | "settings" | "keyboard";

// Tabler Icons: Outline, and Filled for the transport and the liked heart.
const GLYPHS: Record<IconName, string> = {
  home: Home,
  compass: Compass,
  search: Search,
  library: Books,
  play: PlayerPlayFilled,
  pause: PlayerPauseFilled,
  skipBack: PlayerSkipBackFilled,
  skipForward: PlayerSkipForwardFilled,
  shuffle: ArrowsShuffle,
  repeat: Repeat,
  repeatOne: RepeatOnce,
  heart: Heart,
  heartFilled: HeartFilled,
  volume: Volume,
  volumeLow: Volume2,
  volumeMute: Volume3,
  chevronLeft: ChevronLeft,
  chevronRight: ChevronRight,
  chevronDown: ChevronDown,
  music: Music,
  user: User,
  logIn: Login,
  logOut: Logout,
  disc: Disc,
  clock: Clock,
  alert: AlertCircle,
  close: X,
  sparkles: Sparkles,
  externalLink: ArrowUpRight,
  info: InfoCircle,
  github: BrandGithub,
  code: Code,
  check: Check,
  sidebarCollapse: LayoutSidebarLeftCollapse,
  sidebarExpand: LayoutSidebarLeftExpand,
  power: Power,
  activity: Activity,
  microphone: Microphone,
  arrowUp: ArrowNarrowUp,
  arrowDown: ArrowNarrowDown,
  settings: Settings,
  keyboard: Keyboard,
};

/** The width of the outline glyphs' lines, of their 24 units. */
const STROKE = 1.5;

const SVG = "data:image/svg+xml,";

/** A glyph's SVG split around its drawing: `[open, body, close]`. */
function split(uri: string): [string, string, string] {
  const svg = decodeURIComponent(uri.slice(SVG.length));
  const open = svg.indexOf(">") + 1;
  const close = svg.lastIndexOf("</");
  return [svg.slice(0, open), svg.slice(open, close), svg.slice(close)];
}

/** The inside of `name`'s SVG (24×24, in `currentColor`). */
const bodyOf = (name: IconName) => split(GLYPHS[name])[1];

/** The speaker alone: the first frame of the volume icons' morph. */
const speaker = () => bodyOf("volumeMute").match(/<path d='M6 15h-2[^>]*>/)?.[0] ?? bodyOf("volumeMute");

/** The glyph at `t` (0..1) through its hover morph: the drawing itself
 * changes, in a way that says what the control does. At 0 and 1, the glyph
 * at rest. */
type Morph = (body: string, t: number) => string;

const ease = (t: number) => (t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2);
const C = 12; // the glyphs' centre, in their 24 units

/** A fading echo trails the glyph the way it skips. */
const trail =
  (dir: number): Morph =>
  (body, t) => {
    const e = env(t);
    return `<g opacity="${(0.45 * e).toFixed(3)}" transform="translate(${(dir * 3.2 * e).toFixed(2)} 0)">${body}</g>${body}`;
  };

const MORPHS: Partial<Record<IconName, [Morph, number]>> = {
  skipForward: [trail(1), 380],
  skipBack: [trail(-1), 380],
  // The crossing lines change places, and back.
  shuffle: [(body, t) => `<g transform="translate(0 ${C}) scale(1 ${Math.cos(2 * Math.PI * ease(t)).toFixed(3)}) translate(0 ${-C})">${body}</g>`, 520],
  // The arrows go once round (half a turn brings the glyph back to itself).
  repeat: [(body, t) => `<g transform="rotate(${(180 * ease(t)).toFixed(2)} ${C} ${C})">${body}</g>`, 480],
  repeatOne: [(body, t) => `<g transform="rotate(${(360 * ease(t)).toFixed(2)} ${C} ${C})">${body}</g>`, 560],
  // One beat.
  heart: [(body, t) => scaled(body, 1 + 0.14 * env(t)), 320],
  heartFilled: [(body, t) => scaled(body, 1 + 0.14 * env(t)), 320],
  // The sound comes back: the speaker alone, then its waves one by one.
  volume: [(body, t) => (t < 0.3 ? speaker() : t < 0.6 ? bodyOf("volumeLow") : body), 420],
  volumeLow: [(body, t) => (t < 0.45 ? speaker() : body), 300],
  volumeMute: [(body, t) => (t < 0.45 ? speaker() : body), 300],
  // The bars press together a little; the triangle leans on.
  pause: [(body, t) => `<g transform="translate(${C} 0) scale(${(1 - 0.18 * env(t)).toFixed(3)} 1) translate(${-C} 0)">${body}</g>`, 300],
  play: [(body, t) => `<g transform="translate(${(1.3 * env(t)).toFixed(2)} 0)">${body}</g>`, 300],
};

function scaled(body: string, k: number): string {
  return `<g transform="translate(${C} ${C}) scale(${k.toFixed(3)}) translate(${-C} ${-C})">${body}</g>`;
}

/** Whether `name` changes its drawing on hover (else it moves as a whole). */
export function morphs(name: IconName): boolean {
  return name in MORPHS;
}

const URIS = new Map<string, string>();
const STEPS = 24;

/** `name`'s glyph at `t` through its hover morph: the set's own at rest. */
function glyphUri(name: IconName, t = 0): string {
  const morph = MORPHS[name];
  const q = morph && t > 0 && t < 1 ? Math.round(t * STEPS) : 0;
  if (!morph || q <= 0 || q >= STEPS) return GLYPHS[name];
  const key = `${name}|${q}`;
  let uri = URIS.get(key);
  if (!uri) {
    const [open, body, close] = split(GLYPHS[name]);
    const svg = open + morph[0](body, q / STEPS) + close;
    uri = SVG + svg.replaceAll("%", "%25").replaceAll("<", "%3C").replaceAll(">", "%3E").replaceAll("#", "%23").replaceAll('"', "%22");
    URIS.set(key, uri);
  }
  return uri;
}

/** A theme token an icon can be drawn in. */
export type IconColor = keyof Tokens & string;

export interface IconProps {
  name: IconName;
  /** Points (default 24). */
  size?: number;
  /** A theme token (default `"foreground"`). */
  color?: IconColor;
  /** A colour of its own (`#rrggbb`), over `color`. */
  hex?: string;
  /** How far through its hover morph (`iconMotion().t()`), 0 at rest. */
  t?: number;
}

/** The token an icon draws in by default. */
const DEFAULT_COLOR: IconColor = "foreground";

/** The colour an icon draws in: `hex`, or its token's value now (reading the
 * appearance, so it follows light and dark). */
function colorOf(props: IconProps): string {
  colorScheme();
  return props.hex || String(token((props.color ?? DEFAULT_COLOR) as never));
}

/** The named glyph, in a theme token's color. Decorative: the control
 * around it says what it does. */
export function Icon(props: IconProps) {
  return (
    <Glyph
      icon={glyphUri(props.name, props.t)}
      size={props.size ?? 24}
      strokeWidth={STROKE}
      class={props.hex ? `text-[${props.hex}]` : `text-${props.color ?? DEFAULT_COLOR}`}
    />
  );
}

/** A glyph swapping for another (transitions.dev's icon swap): both at
 * once, the old one shrinking to a quarter, blurring and fading as the new
 * one grows from a quarter, sharpens and comes in. */
const SWAP_IN: MotionSpec = {
  keyframes: { opacity: [0, 1], scale: [SCALE.swap, 1] },
  duration: DURATION.base,
  easing: EASING.inOut,
};
const SWAP_OUT: MotionSpec = {
  keyframes: { opacity: [1, 0], scale: [1, SCALE.swap] },
  duration: DURATION.base,
  easing: EASING.inOut,
};
/** NSH draws a blur but cannot animate one: it steps, this many times
 * through the swap (a few timeouts per swap, nothing at rest). */
const BLUR_STEPS = 4;
/** The same glyph changing colour (the like heart turning red): the old
 * colour fades off the new one, like transitions.dev's 150ms colour fade. */
const RECOLOR: MotionSpec = {
  keyframes: { opacity: [1, 0] },
  duration: DURATION.fast,
  easing: EASING.out,
};

/** `Icon`, whose glyph swaps when `name` changes (play and pause, the
 * heart, repeat, the volume's levels): the glyph leaving keeps the colour it
 * had. A new colour of the same glyph fades in (`RECOLOR`). Its first
 * glyph just shows; nothing moves at rest or with reduced motion. `t` (the hover morph) is the glyph showing. */
export function SwapIcon(props: IconProps) {
  // Made on the first swap: most glyphs never swap.
  const incoming = lazyMotion();
  const outgoing = lazyMotion();
  /** The glyph going: swapped out (`swap`, blurring) or a colour fading off. */
  const leaving = signal<{ name: IconName; hex: string; swap: boolean } | null>(null);
  /** How far through the swap's blur: `BLUR_STEPS` at rest. */
  const step = signal(BLUR_STEPS);
  let timers: ReturnType<typeof setTimeout>[] = [];
  const stopBlur = () => {
    for (const timer of timers) clearTimeout(timer);
    timers = [];
  };
  let name = untracked(() => props.name);
  let hex = untracked(() => colorOf(props));
  effect(() => {
    const now = props.name;
    const color = colorOf(props);
    if (now === name) {
      if (color === hex) return;
      const was = { name, hex, swap: false };
      hex = color;
      untracked(() => {
        // A swap going on takes the new colour as it is.
        if (leaving() || reduceMotion()) return;
        outgoing.reset();
        leaving.set(was);
        void outgoing.play(RECOLOR).then(({ finished }) => finished && leaving.set(null));
      });
      return;
    }
    const was = { name, hex, swap: true };
    name = now;
    hex = color;
    untracked(() => {
      stopBlur();
      incoming.stop();
      outgoing.stop();
      if (reduceMotion()) {
        batch(() => {
          leaving.set(null);
          step.set(BLUR_STEPS);
        });
        incoming.reset();
        return;
      }
      batch(() => {
        leaving.set(was);
        step.set(0);
      });
      void incoming.play(SWAP_IN);
      void outgoing.play(SWAP_OUT).then(({ finished }) => finished && leaving.set(null));
      for (let i = 1; i <= BLUR_STEPS; i++) {
        timers.push(setTimeout(() => step.set(i), (DURATION.base * i) / BLUR_STEPS));
      }
    });
  });
  cleanup(() => {
    stopBlur();
    incoming.stop();
    outgoing.stop();
  });
  const size = () => props.size ?? 24;
  const blur = (amount: number) => (amount > 0 ? `blur(${amount.toFixed(2)}px)` : undefined);
  return (
    <View class="items-center justify-center" style={{ width: size(), height: size() }}>
      <View style={incoming.style}>
        <View style={{ filter: blur(BLUR.small * (1 - step() / BLUR_STEPS)) }}>
          <Icon name={props.name} size={size()} color={props.color} hex={props.hex} t={props.t} />
        </View>
      </View>
      <If when={leaving()}>
        {(was) => (
          <View class="absolute inset-0 items-center justify-center" style={outgoing.style}>
            <View style={{ filter: was().swap ? blur((BLUR.small * step()) / BLUR_STEPS) : undefined }}>
              <Icon name={was().name} size={size()} hex={was().hex} />
            </View>
          </View>
        )}
      </If>
    </View>
  );
}

/** A glyph's one hover move: where it is (points, degrees) at `t`, 0..1
 * through the move. Each rides a half-sine envelope, so it leaves rest and
 * comes back to it without a jump; the amplitudes are small, felt more than
 * seen (after allsum's sidebar icons). */
type Pose = (t: number) => { x?: number; y?: number; rotate?: number; scale?: number };

const env = (t: number) => Math.sin(t * Math.PI);
const deg = (rad: number) => (rad * 180) / Math.PI;

/** Up 2 points with a slight lean back (300 ms). */
const lift: Pose = (t) => ({ y: -2 * env(t), rotate: deg(-0.12 * env(t)) });
/** Two swings about the centre, dying away (480 ms). */
const swing: Pose = (t) => ({ rotate: deg(Math.sin(t * 4 * Math.PI) * env(t) * 0.18) });
/** Out along a direction and back (300 ms). */
const nudge =
  (dx: number, dy: number): Pose =>
  (t) => ({ x: dx * env(t), y: dy * env(t) });
/** A quick double beat (420 ms). */
const beat: Pose = (t) => ({ scale: 1 + 0.14 * Math.abs(Math.sin(t * 2 * Math.PI)) * env(t) });
/** A part turn and back (420 ms). */
const turn =
  (by: number): Pose =>
  (t) => ({ rotate: by * env(t) });
/** A small scribble: side to side twice, lifting (560 ms). */
const scribble: Pose = (t) => {
  const stroke = Math.sin(t * 4 * Math.PI);
  return { x: stroke * 1.5 * env(t), y: -Math.abs(stroke) * 0.6 * env(t), rotate: deg(-0.2 * env(t)) };
};

const MOVES: Partial<Record<IconName, [Pose, number]>> = {
  home: [lift, 300],
  compass: [swing, 480],
  search: [swing, 480],
  library: [lift, 300],
  chevronLeft: [nudge(-2, 0), 300],
  chevronRight: [nudge(2, 0), 300],
  chevronDown: [nudge(0, 2), 300],
  logIn: [nudge(2, 0), 300],
  logOut: [nudge(2, 0), 300],
  externalLink: [nudge(1.5, -1.5), 300],
  user: [lift, 300],
  sparkles: [swing, 480],
  activity: [lift, 300],
  microphone: [lift, 300],
  info: [lift, 300],
  github: [swing, 480],
  code: [scribble, 560],
  close: [turn(90), 300],
  settings: [turn(45), 320],
  keyboard: [lift, 300],
};

/** Samples a pose into keyframes the native animation plays (linear
 * between samples: the pose has its own easing). */
function spec(pose: Pose, duration: number): MotionSpec {
  const steps = Math.max(8, Math.round(duration / 16));
  const frames = Array.from({ length: steps + 1 }, (_, i) => {
    const p = pose(i / steps);
    return { translateX: p.x ?? 0, translateY: p.y ?? 0, rotate: `${p.rotate ?? 0}deg`, scale: p.scale ?? 1 };
  });
  return { keyframes: frames, duration, easing: "linear" };
}

const SPECS = new Map<IconName, MotionSpec>();
function specOf(name: IconName): MotionSpec {
  let s = SPECS.get(name);
  if (!s) {
    const [pose, duration] = MOVES[name] ?? [lift, 300];
    s = spec(pose, duration);
    SPECS.set(name, s);
  }
  return s;
}

/** A glyph's hover move: spread `style` on a view around the icon and call
 * `play(name)` as the pointer enters the control (`onMouseEnter`). Each
 * entry plays it once from rest, ending at rest; nothing when the user
 * asked for less motion. */
export function iconMotion() {
  // Made on the first hover: most glyphs are never hovered.
  const motion = lazyMotion();
  const t = signal(0);
  let timer: ReturnType<typeof setInterval> | null = null;
  const stopMorph = () => {
    if (timer !== null) clearInterval(timer);
    timer = null;
    t.set(0);
  };
  try {
    cleanup(stopMorph);
  } catch {
    // Outside a component: nothing to stop with.
  }
  return {
    /** Bind it in a binding (`style={hover.style}`): it follows. */
    get style() {
      return motion.style;
    },
    /** How far through the glyph's morph (pass to `Icon`'s `t`). */
    t: () => t(),
    play(name: IconName) {
      if (reduceMotion()) return;
      const morph = MORPHS[name];
      if (morph) {
        stopMorph();
        const start = Date.now();
        timer = setInterval(() => {
          const k = (Date.now() - start) / morph[1];
          if (k >= 1) stopMorph();
          else t.set(k);
        }, 16);
        return;
      }
      motion.stop();
      void motion.play(specOf(name));
    },
  };
}
