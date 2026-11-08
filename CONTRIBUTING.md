# Contributing to Spotish

Contributions are welcome. If you find a bug or have an idea, please open an issue or a pull request.

Please write clean code that follows the style and organization of the existing code, and make sure it builds
(`cargo check`) and the tests pass (`cargo test`).

Please submit **one PR that does one thing** — it is easier to review and faster to merge.

## Project layout

- `src/spotify/` — the engine: it runs and drives Spotify's own web player in a headless browser over the DevTools
  protocol (`chrome.rs`, `cdp.rs`, `session.rs`), and the data it returns (`model.rs`). This layer is Rust.
- `src/store.rs` — the app's state, the one owner of what the engine reports and the view's history.
- `src/ui/` — the app's own styled component library: theme tokens, icons, controls, motion and the ambient shader.
- `src/components/`, `src/pages/`, `src/shell.rs` — the screens, composed from `src/ui/` and `nsh_base`.
- `src/bridge/bridge.js` — the script installed in the web player: it reads what the player uses to talk to Spotify
  and reports the player's state. Every selector and query name the web player exposes is read in this one file, so a
  change to the web player is mended here.

## Built on NSH

Spotish is built with [NSH (Native Shell)](https://nsh.dev). Read its coding and design guides before changing the UI;
draw with theme tokens, wrap each `nsh/base` component once in `src/ui/`, and keep the look the app's own.

## Respecting Spotify

Spotish plays through Spotify's own web player and **never blocks or skips Spotify's ads**. Keep it that way: features
that remove ads or circumvent Spotify's free-tier limits will not be merged. Point listeners to Spotify Premium instead.
