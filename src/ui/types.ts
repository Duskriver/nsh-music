// The shapes the engine's data methods return (the web player's JSON, passed
// straight through by `spotish/native`). The engine types them as `unknown`;
// these are the agreed interfaces the UI reads them as.

/** A playlist, album, artist, show, Liked Songs or a track shown as a card. */
export interface Item {
  uri: string;
  /** `playlist`, `album`, `artist`, `show`, `collection` or `track`. */
  kind: string;
  name: string;
  subtitle: string;
  image: string;
}

/** A track in a list. */
export interface Track {
  uri: string;
  uid: string;
  name: string;
  artists: string;
  album: string;
  albumUri: string;
  image: string;
  durationMs: number;
  explicit: boolean;
  playable: boolean;
}

/** The header of a collection's page. */
export interface Header {
  uri: string;
  kind: string;
  name: string;
  subtitle: string;
  image: string;
  description: string;
}

/** A playlist, album, artist or Liked Songs: header, tracks and (artist) releases. */
export interface Collection {
  header: Header;
  tracks: Track[];
  releases: Item[];
  total: number;
}

/** A titled row of cards. */
export interface Section {
  title: string;
  items: Item[];
}

/** The home page. */
export interface Home {
  greeting: string;
  sections: Section[];
}

/** What a search found. */
export interface SearchResults {
  tracks: Track[];
  artists: Item[];
  albums: Item[];
  playlists: Item[];
}

/** The signed-in user's library. */
export interface Library {
  items: Item[];
}

/** The signed-in user. */
export interface Me {
  name: string;
  username: string;
  image: string;
}

/** Whether an item has a page (a track has none — it plays). */
export function opens(item: Item): boolean {
  return item.kind === "playlist" || item.kind === "album" || item.kind === "artist" || item.kind === "collection";
}

/** `m:ss` (or `h:mm:ss`) of a duration in milliseconds. */
export function clock(ms: number): string {
  const seconds = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds / 60) % 60);
  const s = seconds % 60;
  const pad = (n: number) => String(n).padStart(2, "0");
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${m}:${pad(s)}`;
}
