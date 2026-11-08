// The app's store: the engine (the Spotify class), a reactive mirror of the
// player's state, the view's history, and the transient view state. One owner
// per piece of state — the engine owns playback, TSX mirrors it in signals and
// changes it only through the engine's methods.

import { type Accessor, batch, signal, type Signal } from "nsh";
import { Spotify, type Health, type Player } from "spotish/native";

/** A page of the main view. */
export type Route = { name: "home" } | { name: "search" } | { name: "collection"; uri: string };

const EMPTY_PLAYER: Player = {
  signedIn: false,
  title: "",
  artist: "",
  album: "",
  artwork: "",
  trackUri: "",
  contextHref: "",
  playing: false,
  canPlay: false,
  positionMs: 0,
  durationMs: 0,
  shuffle: false,
  shuffleAvailable: false,
  repeat: "off",
  volume: 1,
  muted: false,
  liked: false,
  canNext: false,
  canPrevious: false,
  ad: false,
  premium: false,
};

export interface Store {
  readonly spotify: Spotify;
  // Reactive mirrors of the engine.
  readonly player: Accessor<Player>;
  readonly health: Accessor<Health>;
  readonly signedIn: Accessor<boolean | null>;
  /** Bumped when data may load anew (player ready, signed in/out, token). */
  readonly epoch: Accessor<number>;
  // The view.
  readonly route: Accessor<Route>;
  readonly canBack: Accessor<boolean>;
  readonly canForward: Accessor<boolean>;
  // Transient view state.
  readonly query: Signal<string>;
  readonly seeking: Signal<number | null>;
  readonly volumeDraft: Signal<number | null>;
  readonly nowPlaying: Signal<boolean>;
  readonly about: Signal<boolean>;
  readonly drawer: Signal<boolean>;
  readonly notice: Accessor<string | null>;

  // Navigation.
  open(route: Route): void;
  openAndCloseDrawer(route: Route): void;
  back(): void;
  forward(): void;

  // Playback (optimistic where it helps; errors surface as a notice).
  toggle(): void;
  playItem(item: { uri: string; kind: string }): void;
  playAt(uri: string, index: number): void;
  playTrack(uri: string): void;
  control(name: ControlName, run: () => Promise<unknown>): void;
  seek(ms: number): void;
  setVolume(v: number): void;
  notify(message: string): void;

  // Account and links.
  signIn(): void;
  cancelSignIn(): void;
  finishSignIn(): void;
  signOut(): void;
  openPremium(): void;
  openExternal(url: string): void;
}

type ControlName = "toggle" | "next" | "previous" | "shuffle" | "repeat" | "like" | "mute" | "seek" | "volume";

function label(name: ControlName): string {
  switch (name) {
    case "next": return "skip";
    case "previous": return "go back";
    case "shuffle": return "change shuffle";
    case "repeat": return "change repeat";
    case "like": return "save the track";
    case "mute":
    case "volume": return "change the volume";
    case "seek": return "seek";
    default: return "play or pause";
  }
}

export function createStore(): Store {
  const spotify = new Spotify();

  const player = signal<Player>(spotify.player ?? EMPTY_PLAYER);
  const health = signal<Health>(spotify.health ?? { status: "starting", error: null });
  const signedIn = signal<boolean | null>(spotify.signedIn ?? null);
  const epoch = signal(0);

  const query = signal("");
  const seeking = signal<number | null>(null);
  const volumeDraft = signal<number | null>(null);
  const nowPlaying = signal(false);
  const about = signal(false);
  const drawer = signal(false);
  const notice = signal<string | null>(null);

  const routes = signal<Route[]>([{ name: "home" }]);
  const at = signal(0);

  // Mirror the engine. Subscribe first, then start pumping events.
  spotify.onPlayer((p) => player.set(p));
  spotify.onHealth((h) => {
    const wasReady = health().status === "ready";
    health.set(h);
    if (h.status === "ready" && !wasReady) epoch.update((e) => e + 1);
  });
  spotify.onSignedIn((v) => {
    if (signedIn() !== v) {
      signedIn.set(v);
      epoch.update((e) => e + 1);
    }
  });
  spotify.onAuth(() => epoch.update((e) => e + 1));
  spotify.start();

  let noticeTimer: ReturnType<typeof setTimeout> | undefined;
  const notify = (message: string) => {
    notice.set(message);
    if (noticeTimer) clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => notice.set(null), 5000);
  };

  const canPlay = (): boolean => {
    if (health().status !== "ready") {
      notify("The player is still starting.");
      return false;
    }
    if (signedIn() !== true) {
      notify("Log in to Spotify to play music.");
      return false;
    }
    return true;
  };

  const control = (name: ControlName, run: () => Promise<unknown>) => {
    if (!canPlay()) return;
    run().catch((error) => notify(`Couldn't ${label(name)}: ${error}`));
  };

  const play = (run: () => Promise<unknown>) => {
    if (!canPlay()) return;
    notify("Starting playback…");
    run().then(
      () => notice.set(null),
      (error) => notify(`Couldn't play that: ${error}`),
    );
  };

  const open = (route: Route) => {
    batch(() => {
      const current = routes()[at()];
      if (current.name === route.name && (route.name !== "collection" || (current as { uri: string }).uri === route.uri)) {
        return;
      }
      routes.update((r) => [...r.slice(0, at() + 1), route]);
      at.update((i) => i + 1);
    });
  };

  return {
    spotify,
    player,
    health,
    signedIn,
    epoch,
    route: () => routes()[at()],
    canBack: () => at() > 0,
    canForward: () => at() + 1 < routes().length,
    query,
    seeking,
    volumeDraft,
    nowPlaying,
    about,
    drawer,
    notice,

    open,
    openAndCloseDrawer: (route) => {
      open(route);
      drawer.set(false);
    },
    back: () => at.update((i) => Math.max(0, i - 1)),
    forward: () => at.update((i) => Math.min(routes().length - 1, i + 1)),

    toggle: () => {
      if (!canPlay()) return;
      player.update((p) => ({ ...p, playing: !p.playing }));
      spotify.toggle().catch((error) => notify(`Couldn't play or pause: ${error}`));
    },
    playItem: (item) => {
      // Free accounts can't play a single track on demand; play its context.
      if (item.kind === "track" && player().premium) play(() => spotify.playTrack(item.uri));
      else if (item.kind === "track") play(() => spotify.playTrack(item.uri));
      else play(() => spotify.playContext(item.uri, null));
    },
    playAt: (uri, index) => {
      if (player().premium) {
        play(() => spotify.playContext(uri, index));
      } else {
        // Spotify Free shuffles a playlist rather than playing a chosen track.
        notify("Spotify Free shuffles playlists — Premium lets you pick any song.");
        play(() => spotify.playContext(uri, null));
      }
    },
    playTrack: (uri) => play(() => spotify.playTrack(uri)),
    control,
    seek: (ms) => control("seek", () => spotify.seek(ms)),
    setVolume: (v) => control("volume", () => spotify.setVolume(v)),
    notify,

    signIn: () => spotify.signIn(),
    cancelSignIn: () => spotify.cancelSignIn(),
    finishSignIn: () => spotify.finishSignIn(),
    signOut: () => spotify.signOut(),
    openPremium: () => void spotify.openPremium(),
    openExternal: (url) => void spotify.openExternal(url),
  };
}

/** The app-wide store (one per run; persists across TSX hot updates). */
export const store = createStore();
