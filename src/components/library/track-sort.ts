// How a playlist's tracks are shown: ordered as the playlist has them or by
// one of the built-in sorts, and as a list or compact. Only what is shown
// changes: playing a track still plays the playlist in its original order
// from that track.

import { signal, Storage } from "nsh";
import { t } from "../../i18n";
import type { Track } from "../../backend/types";

export type TrackSort = "custom" | "title" | "artist" | "album" | "added" | "release" | "duration";
export type TrackView = "list" | "compact";

const TRACK_VIEW_KEY = "ui:trackView";

/** The chosen order, whether it runs against its usual direction, and the
 * view, kept while the app runs (view persisted across runs). */
export const trackSort = signal<TrackSort>("custom");
export const trackReversed = signal(false);
export const trackView = signal<TrackView>("list");

Storage.getItem(TRACK_VIEW_KEY)
  .then((saved) => {
    if (saved === "list" || saved === "compact") trackView.set(saved);
  })
  .catch(() => {});

/** Switches the playlist track row density and persists it. */
export function setTrackView(view: TrackView): void {
  trackView.set(view);
  Storage.setItem(TRACK_VIEW_KEY, view).catch(() => {});
}

/** Picks `order`; picking the one in use again turns it round. Custom order
 * has no other direction. */
export function chooseSort(order: TrackSort): void {
  if (order === trackSort() && order !== "custom") {
    trackReversed.set(!trackReversed());
    return;
  }
  trackSort.set(order);
  trackReversed.set(false);
}

/** The way `order` runs: title, artist, album and duration up (A to Z,
 * shortest first), recently added and release date down (newest first),
 * each turned round when `reversed`. */
export function directionOf(order: TrackSort, reversed: boolean): "asc" | "desc" {
  const usual = order === "added" || order === "release" ? "desc" : "asc";
  return reversed ? (usual === "asc" ? "desc" : "asc") : usual;
}

/** The order ids offered by the sort menu, in display order. */
export const TRACK_SORT_IDS: readonly TrackSort[] = ["custom", "title", "artist", "album", "added", "release", "duration"];

/** Localized label for a sort order (tracked by current locale). */
export function sortLabel(order: TrackSort): string {
  return t(`sort.orders.${order}`);
}

/** Localized label for a track density view (tracked by current locale). */
export function viewLabel(view: TrackView): string {
  return t(`sort.views.${view}`);
}

/** A time in ms, or -Infinity when `iso` is not a date (so it sorts last). A
 * bare year counts as its first day. */
function time(iso: string | undefined): number {
  const ms = Date.parse(iso ?? "");
  return Number.isNaN(ms) ? -Infinity : ms;
}

/** Words in order, ignoring case, with a run of digits read as one number
 * ("c9" before "c10"). Done by hand: the engine's `localeCompare` takes no
 * options. */
function words(a: string, b: string): number {
  const parts = (text: string) => text.toLowerCase().match(/\d+|\D+/g) ?? [];
  const left = parts(a);
  const right = parts(b);
  for (let i = 0; i < Math.min(left.length, right.length); i++) {
    const x = left[i];
    const y = right[i];
    const bothNumbers = /^\d/.test(x) && /^\d/.test(y);
    const order = bothNumbers ? Number(x) - Number(y) : x < y ? -1 : x > y ? 1 : 0;
    if (order !== 0) return order;
  }
  return left.length - right.length;
}

/** `tracks` in `order`, turned round when `reversed`; the playlist's order is
 * returned as it came. A tie stays in the playlist's order, and a track with
 * no date comes last in either direction. */
export function sortTracks(tracks: Track[], order: TrackSort, reversed = false): Track[] {
  if (order === "custom") return tracks;
  const sign = reversed ? -1 : 1;
  // Newest first by default; undated last whichever way it runs.
  const byTime = (a: number, b: number): number => {
    if (a === b) return 0;
    if (a === -Infinity) return 1;
    if (b === -Infinity) return -1;
    return (b - a) * sign;
  };
  const compare = (a: Track, b: Track): number => {
    switch (order) {
      case "title":
        return words(a.name, b.name) * sign;
      case "artist":
        return words(a.artists, b.artists) * sign;
      case "album":
        return words(a.album, b.album) * sign;
      case "added":
        return byTime(time(a.addedAt), time(b.addedAt));
      case "release":
        return byTime(time(a.releaseDate), time(b.releaseDate));
      case "duration":
        return (a.durationMs - b.durationMs) * sign;
    }
  };
  return tracks
    .map((track, index) => ({ track, index }))
    .sort((a, b) => compare(a.track, b.track) || a.index - b.index)
    .map((entry) => entry.track);
}
