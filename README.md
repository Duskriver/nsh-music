# Spotish

**A native player for Spotify, built with [NSH](https://nsh.dev).**

Spotish is a lightweight, native desktop client for Spotify. Instead of running Spotify's web app (PWA) in a full
browser, it wraps Spotify's own web player — run out of sight and driven by the app — behind a fast, native interface
drawn by NSH. Your Spotify account, free or Premium, does the playing, so Spotify's catalogue, your library, and its
ads all work exactly as Spotify intends.

> Spotish is an **unofficial, personal project**. It is **not affiliated with, endorsed by, or sponsored by Spotify**.
> Spotify, its logo, and all music, artwork and trademarks belong to Spotify and the respective rights holders. A
> Spotify account is required, and your use of Spotify through this app is subject to
> [Spotify's Terms of Service](https://www.spotify.com/legal/end-user-agreement/).
>
> **Spotish never blocks or skips Spotify's ads.** [Spotify Premium](https://www.spotify.com/premium/) is how you remove
> them, play any track on demand, and support the artists you listen to.

## What is NSH?

[NSH (Native Shell)](https://nsh.dev) is a Rust UI framework that renders one app through each platform's own native
views — on macOS, Windows, Linux, iOS, Android, the web and even the terminal. Spotish's whole interface — its layout,
lists, animations and the live shader behind Now Playing — is drawn natively by NSH, not in a web page. The project is
also a showcase of what NSH can do.

## How it works

- The **engine** (`src/spotify/`) starts a headless Chromium or Google Chrome, loads Spotify's web player, and drives
  it over the Chrome DevTools Protocol. Spotify's audio is DRM-protected (Widevine), which Chromium decodes; the app
  reuses the Widevine component an existing Chromium/Chrome profile already has.
- A small **bridge** (`src/bridge/bridge.js`) runs inside the web player. It reads what the player itself uses to talk
  to Spotify (the headers and persisted queries of its own requests), so the app can show the same data, and it reports
  the player's state and presses its controls.
- The **UI** (`src/ui/`, `src/screens/`, `App.tsx`) is all native NSH, written in TSX over a Rust engine: a Spotify-style sidebar workspace that
  adapts from a desktop layout to a phone layout, a player bar, a full-screen Now Playing view with an ambient shader,
  and a library, search and collection pages.

## Requirements

- **Chromium** or **Google Chrome** installed (set `SPOTISH_BROWSER` to override the path).
- A **Spotify account** (free or Premium).
- Linux, macOS or Windows. Playback needs the Widevine CDM, which Chrome ships and Chromium downloads on first run.

## Build and run

```sh
nsh run                # run on this machine (builds the Rust engine, bundles the TSX UI)
nsh build              # type-check and bundle
nsh check              # cargo check + tsc
```

Set `SPOTISH_DEMO=1` to run against recorded sample data with no browser (for trying the UI).

You sign in once — the login is saved in Spotish's own browser profile and kept across restarts. To skip even
that and reuse a browser you are already signed in to, set `SPOTISH_PROFILE` to that browser's profile
directory (for example `~/.config/chromium`); that browser must be closed while Spotish runs.

On first launch, press **Log in** and sign in to Spotify in the window that opens (your account, or Google, Apple or
Facebook); then press **I'm logged in**, and Spotish takes over playback.

## Benchmarks

_TODO: measured footprint (native UI vs a full web player) and startup/frame numbers._

## Playback and Spotify Free

Spotish plays through your Spotify account. **Spotify Free** can shuffle-play playlists,
albums and artists but cannot play a chosen track on demand — so tapping a song shuffles
its playlist (as the web player does). **Premium** plays any track on demand and removes ads.

## License

[Apache-2.0](LICENSE-APACHE). As a special grant, **Spotify AB may use this project freely, with no
conditions and no attribution required** — see [NOTICE.md](NOTICE.md).
