// The app's queries: each a cache-first load keyed by the account, loading
// again with each new epoch (ready, signed in or out, a new token).

import { type Accessor, resource } from "nsh";
import { type CachedQuery, cachedQuery } from "./cache";
import { type Collection, type Home, kindOf, type Library, type Me } from "../backend/types";
import { backend } from "../state/engine";
import { cacheAccount, dataEpoch } from "../state/session";

function scoped(key: () => string | null) {
  return () => {
    const account = cacheAccount();
    const k = key();
    if (account === null || k === null) return null;
    return { account, key: k, epoch: dataEpoch() };
  };
}

/** The home page. */
export function homeQuery(): CachedQuery<Home> {
  return cachedQuery(scoped(() => "home"), () => backend.home());
}

/** A playlist, album, artist or Liked Songs by URI. */
export function collectionQuery(uri: () => string): CachedQuery<Collection> {
  return cachedQuery(scoped(uri), (key) => {
    switch (kindOf(key)) {
      case "album":
        return backend.album(key);
      case "artist":
        return backend.artist(key);
      case "collection":
        return backend.likedSongs();
      default:
        return backend.playlist(key);
    }
  });
}

/** The signed-in user's library (nothing while signed out). */
export function libraryQuery(): CachedQuery<Library> {
  return cachedQuery(
    scoped(() => (backend.signedIn() === true ? "library" : null)),
    () => backend.library(),
  );
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
