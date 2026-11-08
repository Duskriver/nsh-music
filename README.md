# Spotish

**An unofficial, native client for Spotify, built with [NSH](https://nsh.dev).**

Spotish is a lightweight, native desktop app that works with your Spotify account. Instead of running Spotify's web
app in a full browser window, it runs Spotify's own web player out of sight in a browser you already have installed,
and drives it from a fast, native interface drawn by NSH. Your Spotify account, free or Premium, does the playing, so
the catalogue, your library and Spotify's ads all work exactly as Spotify intends.

> **Unofficial — not affiliated with, endorsed by or sponsored by Spotify.** Spotify is a trademark of Spotify AB.
> All music and artwork belong to their rights holders. A Spotify account is required, and your use of Spotify
> through this app is subject to [Spotify's Terms of Use](https://www.spotify.com/legal/end-user-agreement/).
>
> **Spotish never blocks or skips Spotify's ads.** [Spotify Premium](https://www.spotify.com/premium/) is how you
> remove them, play any track on demand, and support the artists you listen to.

## What is NSH?

[NSH (Native Shell)](https://nsh.dev) is a Rust UI framework that renders one app through each platform's own native
views — on macOS, Windows, Linux, iOS, Android, the web and even the terminal. Spotish is an experiment that proves
out NSH's cross-platform ability on a real, non-trivial app: one Rust engine and one TSX interface, drawn natively, not
in a web page.

## Platforms

| Platform | Status |
| --- | --- |
| Linux, macOS | Plays music |
| Windows | Plays music through Microsoft Edge |
| Terminal (TUI) | Plays music, through the same engine as the desktop |
| Web, iOS, Android | Demo with sample data; a remote control for your desktop Spotish is planned |

Spotify's free tier plays only in Spotify's web player, with DRM (Widevine), in a real desktop browser. Web pages and
mobile apps cannot start that browser, and Spotify's mobile and web SDKs are for Premium only — so on those platforms
Spotish is a demo.

## Requirements

- An installed **Google Chrome, Chromium, Microsoft Edge or Brave**. Widevine, the DRM module that decodes Spotify's
  audio, cannot be bundled with Spotish; it comes with your browser (Chromium downloads it on first run). Set
  `SPOTISH_BROWSER` to the browser's executable to choose one.
- A **Spotify account** (free or Premium).

## How it works

- The **engine** (`src/spotify/`, Rust) starts your browser without a window, loads Spotify's web player, and drives it
  over the Chrome DevTools Protocol.
- A small **bridge** (`src/bridge/bridge.js`) runs inside the web player. It reads what the player itself uses to talk
  to Spotify, so the app can show the same data, reports the player's state, and presses its controls.
- The **interface** (TSX, from `App.tsx`) is native NSH: a sidebar workspace that adapts from a desktop layout to a
  phone layout, a player bar, a full-screen Now Playing view, library, search and collection pages.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the layout of the code.

## Build and run

You need the `nsh` CLI (nightly) and Rust; see [CONTRIBUTING.md](CONTRIBUTING.md#setup).

```sh
nsh run      # run on this machine (builds the Rust engine, bundles the TSX interface)
nsh check    # cargo check + tsc
nsh test     # cargo test + TSX tests
nsh build    # describe the crate and bundle
```

`SPOTISH_DEMO=1 nsh run` runs on recorded sample data, with no browser and no account.

On first launch, press **Log in** and sign in to Spotify in the window that opens (with your Spotify account, or
Google, Apple or Facebook). Spotish keeps the login in its own browser profile across restarts. To reuse a browser you
are already signed in to, set `SPOTISH_PROFILE` to that browser's profile directory (for example `~/.config/chromium`);
close that browser while Spotish runs.

## Playback on Spotify Free

Spotify Free can shuffle-play playlists, albums and artists but cannot play a chosen track on demand, so tapping a
song shuffles its playlist, as Spotify's web player does. Ads play as Spotify serves them. **Premium** plays any track
on demand and has no ads.

## Benchmarks

_TODO: measured footprint (native interface vs a full web player), startup and frame numbers._

## License

[Apache-2.0](LICENSE-APACHE). As a special grant, **Spotify AB may use this project freely, with no conditions and no
attribution required** — see [NOTICE.md](NOTICE.md).
