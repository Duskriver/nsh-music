// The app's queries: each a cache-first load keyed by the account, loading
// again with each new epoch (ready, signed in or out, a new token).

import { type Accessor, resource, type Resource, signal } from "nsh";
import { type CachedQuery, cachedQuery } from "./cache";
import { type Collection, type Home, type Item, kindOf, type Library, type Lyrics, type Me } from "../backend/types";
import { backend } from "../state/engine";
import { cacheAccount, dataEpoch } from "../state/session";

const DAILY_URI = "netease:playlist:daily";
const likedEpoch = signal(0);
const dailyCover = signal<{ account: string; image: string } | null>(null);

function syncDailyCover(account: string | null, image: string | undefined): void {
  if (!account || !image || account !== cacheAccount()) return;
  const cur = dailyCover();
  if (cur?.account === account && cur.image === image) return;
  dailyCover.set({ account, image });
}

function patchItemsDailyCover(items: Item[], image: string): Item[] | null {
  let changed = false;
  const next = items.map((item) => {
    if (item.uri !== DAILY_URI || item.image === image) return item;
    changed = true;
    return { ...item, image };
  });
  return changed ? next : null;
}

function withDailyCoverLibrary(lib: Library | undefined): Library | undefined {
  if (!lib) return lib;
  const account = cacheAccount();
  const override = dailyCover();
  if (!account || override?.account !== account || !override.image) return lib;
  const items = patchItemsDailyCover(lib.items, override.image);
  return items ? { ...lib, items } : lib;
}

function withDailyCoverHome(home: Home | undefined): Home | undefined {
  if (!home) return home;
  const account = cacheAccount();
  const override = dailyCover();
  if (!account || override?.account !== account || !override.image) return home;
  let changed = false;
  const sections = home.sections.map((section) => {
    const items = patchItemsDailyCover(section.items, override.image);
    if (!items) return section;
    changed = true;
    return { ...section, items };
  });
  return changed ? { ...home, sections } : home;
}

/** Triggers a refetch of Liked Songs queries after liking or unliking a track. */
export function invalidateLikedSongs(): void {
  likedEpoch.update((n) => n + 1);
}

function scoped(key: () => string | null, extraEpoch?: (key: string) => number) {
  return () => {
    const account = cacheAccount();
    const k = key();
    if (account === null || k === null) return null;
    return { account, key: k, epoch: dataEpoch() + (extraEpoch ? extraEpoch(k) : 0) };
  };
}

/** The home page. */
export function homeQuery(): CachedQuery<Home> {
  const query = cachedQuery(scoped(() => "home"), async () => {
    const account = cacheAccount();
    const home = await backend.home();
    for (const section of home.sections) {
      const daily = section.items.find((item) => item.uri === DAILY_URI);
      if (daily?.image) {
        syncDailyCover(account, daily.image);
        break;
      }
    }
    return home;
  });
  return {
    ...query,
    data: () => withDailyCoverHome(query.data()),
  };
}

/** A playlist, album, artist or Liked Songs by URI. */
export function collectionQuery(uri: () => string | null): CachedQuery<Collection> {
  return cachedQuery(
    scoped(uri, (k) => (kindOf(k) === "collection" ? likedEpoch() : 0)),
    async (key) => {
      const account = cacheAccount();
      switch (kindOf(key)) {
        case "album":
          return backend.album(key);
        case "artist":
          return backend.artist(key);
        case "collection":
          return backend.likedSongs();
        default: {
          const collection = await backend.playlist(key);
          if (key === DAILY_URI) syncDailyCover(account, collection.header.image);
          return collection;
        }
      }
    },
  );
}

/** The signed-in user's library (nothing while signed out). */
export function libraryQuery(): CachedQuery<Library> {
  const query = cachedQuery(
    scoped(() => (backend.signedIn() === true ? "library" : null)),
    async () => {
      const account = cacheAccount();
      const lib = await backend.library();
      const daily = lib.items.find((item) => item.uri === DAILY_URI);
      syncDailyCover(account, daily?.image);
      return lib;
    },
  );
  return {
    ...query,
    data: () => withDailyCoverLibrary(query.data()),
  };
}

/** The signed-in user (nothing while signed out). */
export function meQuery(): CachedQuery<Me> {
  return cachedQuery(
    scoped(() => (backend.signedIn() === true ? "me" : null)),
    () => backend.me(),
  );
}

/** A cover's colours (`#rrggbb`), most telling first, or [] while they
 * are not known or the cover has none. Not cached: the visuals fall back on
 * the theme. */
export function coverColorsQuery(image: () => string): Accessor<string[]> {
  const colors = resource(
    () => image() || null,
    (src) => backend.coverColors(src).catch(() => [] as string[]),
  );
  return () => (image() ? (colors() ?? []) : []);
}

/** A cover's leading colour, or "" (see `coverColorsQuery`). */
export function coverColorQuery(image: () => string): Accessor<string> {
  const colors = coverColorsQuery(image);
  return () => colors()[0] ?? "";
}

/** Lyrics kept in memory, by track URI, most recent last: going back to a
 * song shows its lyrics at once. Not in the stored cache (its room is the
 * pages'), and not per account (lyrics are the same for everyone). */
const lyricsMemo = new Map<string, Lyrics>();
const LYRICS_KEPT = 30;

async function loadLyrics(uri: string): Promise<Lyrics> {
  const known = lyricsMemo.get(uri);
  if (known) {
    lyricsMemo.delete(uri);
    lyricsMemo.set(uri, known);
    return known;
  }
  const lyrics = await backend.lyrics(uri);
  lyricsMemo.set(uri, lyrics);
  while (lyricsMemo.size > LYRICS_KEPT) lyricsMemo.delete(lyricsMemo.keys().next().value as string);
  return lyrics;
}

/** A track's lyrics, with the URI they are of: a resource keeps its last
 * value while the next track's load runs, so readers compare it. Nothing
 * loads without a track; a failed load tries again with the next epoch (a
 * new token). */
export function lyricsQuery(trackUri: () => string): Resource<{ uri: string; lyrics: Lyrics }> {
  return resource(
    () => {
      const uri = trackUri();
      return uri ? { uri, epoch: dataEpoch() } : null;
    },
    async (s) => ({ uri: s.uri, lyrics: await loadLyrics(s.uri) }),
  );
}
