// Motion over NSH's native animation: short entrances that explain where
// content came from. Every motion here honors the user's reduced-motion
// setting (NSH skips it then).
//
// A motion plays when it is created. Create it once, in a component body,
// and bind the style it gives: an `enter(...)` written inside a binding is
// created again, and so played again, whenever that binding re-runs. A
// component from `nsh/base` (a `Button`) reads all its props in one binding,
// so a prop that ticks (`selected` following the player) would replay it.

import {
  Animated,
  type AnimatedValue,
  cleanup,
  type CompositeAnimation,
  createAnimatedValue,
  createMotion,
  type Easing,
  effect,
  LayoutAnimation,
  platform,
  reduceMotion,
  signal,
  untracked,
  type Motion,
  type MotionName,
  type MotionOptions,
  type MotionSpec,
} from "nsh";
import { DISTANCE, DURATION, EASING, SMOOTH_OUT } from "./tokens";

/** A style that plays `preset` once when the view mounts; spread it into
 * `style`. Call it in a component body, or on a plain host view whose style
 * reads nothing else. */
export function enter(preset: MotionName = "fadeInUp", delay = 0) {
  return untracked(() => createMotion({ enter: preset, delay }).style);
}

// ---- Lean motions ----
//
// A list item's entrance moves two channels of a view (a row: its opacity
// and `translateX`; its cover: opacity and `rotateY`). `createMotion` makes
// an animated value for each of its nine channels (and two more nodes for
// the scale), so every cell of every list would cost eleven native nodes
// per motion, made, stopped and released with it. These make only the
// channels that move, and play the same keyframes on them with the same
// timing (`Animated.keyframes` is what a motion runs on each channel).

/** The channels lean motions move, with where each rests. */
const REST = { opacity: 1, translateX: 0, translateY: 0, rotateY: 0 } as const;
type LeanChannel = keyof typeof REST;

/** Keyframes of each channel, played with one timing (`MotionSpec`'s
 * per-channel form). */
interface LeanSpec<C extends LeanChannel> {
  tracks: Record<C, { offset: number; value: number }[]>;
  duration: number;
  easing: Easing;
}

/** Animated values for `channels` only, at rest, and keyframes to play on
 * them; as a `Motion` plays, honouring reduced motion (it lands where it
 * ends). */
function leanMotion<C extends LeanChannel>(channels: readonly C[]) {
  const values = {} as Record<C, AnimatedValue>;
  for (const c of channels) values[c] = createAnimatedValue(REST[c]);
  let running: CompositeAnimation | null = null;
  const stop = () => {
    running?.stop();
    running = null;
  };
  return {
    values,
    play(spec: LeanSpec<C>, delay: number) {
      stop();
      if (reduceMotion()) {
        for (const c of channels) {
          const track = spec.tracks[c];
          values[c].set(track[track.length - 1].value);
        }
        return;
      }
      running = Animated.parallel(
        channels.map((c) =>
          Animated.keyframes(values[c], { keyframes: spec.tracks[c], duration: spec.duration, easing: spec.easing, delay }),
        ),
      );
      void running.start();
    },
    stop,
    /** Stops it and puts every channel back at rest. */
    reset() {
      stop();
      for (const c of channels) values[c].set(REST[c]);
    },
  };
}

/** A motion made the first time it is wanted (`get()`): until then its
 * `style` binds nothing, which looks the same (a motion at rest changes
 * nothing), so a control that may never move (a glyph never hovered, never
 * swapped) makes no native nodes, and none are stopped and released when
 * it goes. Read `style` where it binds (`style={m.style}`): it follows. */
export function lazyMotion(options?: MotionOptions) {
  const made = signal<Motion | null>(null);
  const get = (): Motion => {
    let m = untracked(made);
    if (!m) {
      m = createMotion(options);
      made.set(m);
    }
    return m;
  };
  return {
    /** The motion's style once it is made (tracked). */
    get style() {
      return made()?.style;
    },
    /** The motion, made now if it was not. */
    get,
    /** Plays `motion` (making it first). */
    play(motion: MotionName | MotionSpec, play?: Parameters<Motion["play"]>[1]) {
      return get().play(motion, play);
    },
    /** Stops it where it is (nothing if never made). */
    stop() {
      untracked(made)?.stop();
    },
    /** Puts it back at rest (nothing if never made: it is at rest). */
    reset() {
      untracked(made)?.reset();
    },
  };
}

/** An item sliding in along `axis` from `distance` points away (positive:
 * from the right, or from below) while it fades in, on the long smooth-out
 * curve, passing its place by a hair (1pt) before it settles. A row comes in
 * with `translateX` (NSH's `fadeInRight`: right to left), a card with
 * `translateY` (`fadeInUp`); each as one piece. */
function slideIn<A extends "translateX" | "translateY">(axis: A, distance: number, duration: number): LeanSpec<"opacity" | A> {
  return {
    tracks: {
      opacity: [
        { offset: 0, value: 0 },
        { offset: 0.7, value: 1 },
        { offset: 1, value: 1 },
      ],
      [axis]: [
        { offset: 0, value: distance },
        { offset: 0.7, value: -1 },
        { offset: 1, value: 0 },
      ],
    } as LeanSpec<"opacity" | A>["tracks"],
    duration,
    easing: SMOOTH_OUT,
  };
}

/** A skeleton's pulse (transitions.dev "Skeleton loader and reveal"): its
 * shapes dim to half and back, once a second, while the content loads. */
export const SKELETON_PULSE: MotionSpec = {
  keyframes: { opacity: [1, 0.5, 1] },
  duration: 1000,
  easing: EASING.inOut,
  iterations: "infinite",
};

/** The reveal: the skeleton fades out as the content fades in over it, both
 * over the same time. (The reference also cross-blurs them by 2px; NSH
 * cannot animate a filter, so it is a cross-fade alone.) */
export const REVEAL_IN: MotionSpec = {
  keyframes: { opacity: [0, 1] },
  duration: DURATION.slow,
  easing: EASING.inOut,
};
export const REVEAL_OUT: MotionSpec = {
  keyframes: { opacity: [1, 0] },
  duration: DURATION.slow,
  easing: EASING.inOut,
};

/** The wave lists enter in: each item this long after the one before it
 * (rows top to bottom, cards left to right), unhurried. Only about a
 * screen's worth wait their turn; items past them come in with the last, so
 * the last turn starts within about 1.2s even after a reveal (180 + 11 x 75
 * + 120 = 1125ms). */
export const WAVE = {
  /** Between one item and the next, ms. */
  step: 55,
  /** How many rows down a list take a turn. */
  rows: 12,
  /** How many cards across a row take a turn. */
  cards: 8,
  /** Where an item's wave starts when shown at once (cached). */
  start: 40,
  /** The most an item comes in later than its turn, ms: the irregular
   * rhythm. Over one step and a half, so neighbours can trade places while
   * the wave still runs top to bottom (or left to right) overall. */
  jitter: 90,
} as const;

/** A fixed pseudo-random number in 0..1 for item `index` (and `salt`, for
 * independent draws): the same for the same item on every render, so the
 * irregular rhythm never reshuffles. */
function scatter(index: number, salt: number): number {
  let h = Math.imul(index + 1, 0x9e3779b1) ^ Math.imul(salt + 1, 0x85ebca77);
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d);
  h = Math.imul(h ^ (h >>> 12), 0x297a2d39);
  return ((h ^ (h >>> 15)) >>> 0) / 4294967296;
}

/** A wave item's base duration: between `DURATION.slow` (felt too quick)
 * and `DURATION.slower` (too slow). */
const WAVE_DURATION = Math.round((DURATION.slow + DURATION.slower) / 2);

/** When the item at `index` of a wave of `turns` items starts: its turn plus
 * its own irregular lag. */
function itemDelay(index: number, turns: number): number {
  return waveDelay(index, turns) + Math.round(scatter(index, 0) * WAVE.jitter);
}

/** When items that loaded start their wave: as the skeleton is half gone. */
export const AFTER_REVEAL = DURATION.slow / 2;

/** How long the item at `index` waits in a wave of `turns` items. */
export function waveDelay(index: number, turns: number): number {
  return Math.min(index, turns - 1) * WAVE.step;
}

/** A cover flipping open (NSH's `flipInY`, in our tokens) over `duration`:
 * it turns from edge-on (90°) to face the viewer about its vertical axis,
 * past by a few degrees before it settles, fading in over the first part of
 * the turn. Turning the other way round: its left edge swings in toward
 * the viewer, following the row as it slides in from the right. */
function coverFlipSpec(duration: number): LeanSpec<"opacity" | "rotateY"> {
  return {
    tracks: {
      opacity: [
        { offset: 0, value: 0 },
        { offset: 0.65, value: 1 },
        { offset: 1, value: 1 },
      ],
      rotateY: [
        { offset: 0, value: -90 },
        { offset: 0.65, value: 8 },
        { offset: 1, value: 0 },
      ],
    },
    duration,
    easing: SMOOTH_OUT,
  };
}

/** How far the viewer is from a flipping cover, points: near enough that the
 * turn reads as 3D, far enough that its near edge doesn't balloon. */
const COVER_PERSPECTIVE = 600;

/** How long after its row (or card) starts a cover starts to flip: at
 * least this, plus up to `COVER_LAG_SPREAD` of its own. */
const COVER_LAG = DURATION.instant;
const COVER_LAG_SPREAD = 100;

/** A terminal draws no motion. */
const platformTerminal = () => platform.idiom === "terminal";

// ---- List waves ----
//
// A list's items come in as a wave every time its page is entered (opened,
// or gone back to in the stack), and an item that comes into view later (a
// row scrolled to, a card scrolled across, new search results) comes in on
// its own, with only a little jitter. Each item comes in once per entry: a
// recycled cell given an item already shown, a re-render or a playback tick
// does nothing. An item's motions are made once, in its body; an effect on
// its key and the page's entries plays them.

/** How long after the page is entered (or the content shows) its items
 * still come in as a wave; items shown after that come in on their own. */
const OPENING = 900;
/** The most an item shown later (scrolled to) waits, ms. */
const LATE_JITTER = 60;

/** A list's entrances: one per list (a child per card row), made where the
 * list's content is made. */
export interface ListWave {
  /** How many times the page has been entered (tracked): 0 while it is
   * not shown yet. */
  entry(): number;
  /** Asks to bring item `key` (at `index`) in; `run` gets its delay, ms,
   * after the items asked for in the same moment are put in order. False
   * when it already came in during this entry. */
  request(key: string, index: number, turns: number, run: (delay: number) => void): boolean;
  /** Starts a new wave for items not shown yet (a new search's results). */
  restart(): void;
  /** A wave of its own over the same entries, starting `offset` ms later (a
   * row of cards in a page). */
  child(offset?: number): ListWave;
}

/** Makes a list's wave. `isTop`: whether its page is the one shown
 * (tracked). `afterReveal`: its content took a skeleton's place, so the first
 * wave waits for the skeleton to half fade. */
export function createListWave(options: { isTop: () => boolean; afterReveal?: boolean }): ListWave {
  const made = Date.now();
  const top = untracked(options.isTop);
  const entries = signal(top ? 1 : 0);
  let was = top;
  let windowStart = made;
  effect(() => {
    const now = options.isTop();
    untracked(() => {
      if (now && !was) {
        windowStart = Date.now();
        entries.set(entries() + 1);
      }
      was = now;
    });
  });
  const start = () => (options.afterReveal && untracked(entries) <= 1 && windowStart === made ? AFTER_REVEAL : WAVE.start);
  const make = (offset: number, restartAt: { at: number }): ListWave => {
    const seen = new Map<string, number>();
    let pending: { index: number; turns: number; run: (delay: number) => void }[] = [];
    const flush = () => {
      const batch = pending.sort((a, b) => a.index - b.index);
      pending = [];
      const now = Date.now();
      const opening = now - Math.max(windowStart, restartAt.at) < OPENING;
      batch.forEach((item, rank) => {
        item.run(
          opening
            ? start() + offset + itemDelay(rank, item.turns)
            : Math.round(scatter(item.index, 5) * LATE_JITTER),
        );
      });
    };
    return {
      entry: () => entries(),
      request(key, index, turns, run) {
        const entry = untracked(entries);
        if (entry === 0 || seen.get(key) === entry) return false;
        seen.set(key, entry);
        if (pending.length === 0) void Promise.resolve().then(flush);
        pending.push({ index, turns, run });
        return true;
      },
      restart() {
        restartAt.at = Date.now();
      },
      child(childOffset = 0) {
        return make(offset + childOffset, restartAt);
      },
    };
  };
  return make(0, { at: 0 });
}

/** An item's entrance in its list's wave: the item (`axis`: a row slides in
 * right to left, a card rises) and its cover's flip, each at its own fixed
 * pace (`scatter` of its index), played when `key` first shows in an entry.
 * Call it once, in the item's body; bind `style` on the item and `cover` on
 * a view around its cover. Nothing in a terminal or without a wave. */
export function listItemEntrance(options: {
  wave: ListWave | undefined;
  key: () => string;
  index: () => number;
  axis: "translateX" | "translateY";
  turns: number;
}) {
  const wave = options.wave;
  if (!wave || platformTerminal()) return { style: undefined, cover: undefined };
  const axis = options.axis;
  // Only the channels that move (`leanMotion`): made once per cell.
  const item = leanMotion(["opacity", axis] as const);
  const flip = leanMotion(["opacity", "rotateY"] as const);
  let last = "";
  let lastEntry = -1;
  let seq = 0;
  effect(() => {
    const key = options.key();
    const entry = wave.entry();
    untracked(() => {
      if (key === last && entry === lastEntry) return;
      last = key;
      lastEntry = entry;
      const ticket = ++seq;
      const index = options.index();
      if (!key) return;
      const distance = DISTANCE.large * (0.75 + 0.5 * scatter(index, 1));
      const shown = wave.request(key, index, options.turns, (delay) => {
        if (ticket !== seq) return;
        const duration = Math.round(WAVE_DURATION * (0.92 + 0.16 * scatter(index, 2)));
        const lag = COVER_LAG + Math.round(scatter(index, 3) * COVER_LAG_SPREAD);
        const turn = Math.round(WAVE_DURATION * (0.75 + 0.5 * scatter(index, 4)));
        item.play(slideIn(axis, distance, duration), delay);
        flip.play(coverFlipSpec(turn), delay + lag);
      });
      if (shown) {
        // Hidden where it starts until its turn comes.
        item.values.opacity.set(0);
        item.values[axis].set(distance);
        flip.values.opacity.set(0);
        flip.values.rotateY.set(-90);
      } else {
        // Shown before in this entry (a cell given it back): at rest.
        item.reset();
        flip.reset();
      }
    });
  });
  cleanup(() => {
    item.stop();
    flip.stop();
  });
  // As a motion's style binds them; the channels at rest are left out (they
  // change nothing).
  const slide = item.values[axis];
  return {
    style: { opacity: item.values.opacity, transform: [axis === "translateX" ? { translateX: slide } : { translateY: slide }] },
    cover: { opacity: flip.values.opacity, transform: [{ perspective: COVER_PERSPECTIVE }, { rotateY: flip.values.rotateY }] },
  };
}

/** Animates the layout when `step` changes (a breakpoint is crossed: the
 * sidebar opens or narrows, the player bar or Now Playing rearranges):
 * what moves or resizes glides there, what comes or goes fades. Not on every
 * size change (a window being dragged follows the pointer at once), only
 * when the arrangement does. */
export function animateLayoutOn(step: () => string): void {
  let last: string | null = null;
  effect(() => {
    const now = step();
    if (last !== null && now !== last && !reduceMotion()) {
      const curve = { type: "easeInEaseOut" as const };
      LayoutAnimation.configureNext({
        duration: DURATION.base,
        update: curve,
        create: { ...curve, property: "opacity" },
        delete: { ...curve, property: "opacity" },
      });
    }
    last = now;
  });
}
