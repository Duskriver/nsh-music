// Updates: a newer build of the app's TSX (an OTA package, run from the
// next start) and a newer app (a new binary, for Rust or NSH runtime
// changes), from the GitHub Releases of Duskriver/nsh-music (nsh.json
// "updater"). Quiet: a failed check is logged, and the next check tries
// again.
//
// Wiring (App.tsx, once the app has started; again every few hours if it
// stays open):
//
//   checkForUpdates({ notify: (message, action) => store.notify(message) });
//
// `notify` gets one message per finding, with an action where one makes
// sense ("Restart", "Download"); a toast or a banner shows it.
//
// --- How releases must be made -------------------------------------------
//
// Keys (once; keep the .secret files out of the repository, root.secret
// offline):
//   nsh keygen keys/root          # nsh.json updater.packages.roots ← root.pub
//   nsh keygen keys/release       # signs packages, in CI
//   nsh keygen keys/update        # signs latest.json; nsh.json updater.publicKey ← update.pub
//   nsh certify --root keys/root.secret --key keys/release.pub \
//     --apps io.github.duskriver.nsh-music -o keys/release.cert   # renew yearly
//   gh secret set NSH_RELEASE_KEY < keys/release.secret
//   gh secret set NSH_RELEASE_CERT < keys/release.cert
//   gh secret set NSH_UPDATE_KEY < keys/update.secret
// Until "publicKey" and "packages.roots" are in nsh.json, the app checks for
// new app builds only (unsigned manifests accepted from GitHub) and never
// for packages; only app builds made with them take OTA packages.
//
// Every release raises `build` in nsh.json (one counter for packages and
// app builds) and `version` (semver). Then, on a `v*` tag:
// - TSX only (screens and styles):
//     nsh publish --key release.secret --cert release.cert -o dist/io.github.duskriver.nsh-music-<build>.nshpkg
//     nsh update-manifest --package dist/…nshpkg --notes CHANGES.md --key update.secret -o dist
//   Publish with the nsh version of the app builds in the field, or set
//   nsh.json "minRuntime" to the oldest runtime the TSX supports; when the
//   TSX needs a newer Rust export, raise `Version.api()` (src/version.rs)
//   and gate on it, or ship an app build.
// - Rust, NSH runtime or nsh.json changes: a new app build.
//     nsh package --target linux|macos|windows   # per platform, *-<version>-<os>-<arch>.tar.gz/.zip
//     nsh update-manifest *.tar.gz *.zip [--package …] --notes CHANGES.md --key update.secret
//   On macOS, sign with a Developer ID and notarize before the manifest.
// Upload the archives and/or package, then latest.json and latest.json.sig,
// as assets of a published (not draft, not prerelease) GitHub Release.

import { openUrl } from "nsh";
import { isAvailable as canRestart } from "nsh/desktop";
import * as nshUpdater from "nsh/updater";

/** Where `checkForUpdates` reports what it found. */
export interface UpdateOptions {
  /** Shows `message` to the user, with an optional primary `action`
   * ("Restart", "Download") and a quiet `secondary` choice ("On quit"). */
  notify(
    message: string,
    action?: { label: string; run(): void },
    secondary?: { label: string; run(): void },
  ): void;
  /** Called when a check completes and no package or app update is available. */
  onUpToDate?(): void;
}

/** What `checkForUpdates` uses of `nsh/updater` (tests pass their own). */
export type Updater = Pick<
  typeof nshUpdater,
  | "checkForPackage"
  | "checkForUpdate"
  | "downloadPackage"
  | "downloadUpdate"
  | "installAndRestart"
  | "installOnQuit"
  | "restartToPackage"
>;

/** Where the updates of one `nsh/updater` stand (tests pass their own). */
interface State {
  /** The check in progress, if any: it holds `nsh/updater`, which runs one
   * operation at a time. */
  checking?: Promise<void>;
  /** Whether Restart / On quit is installing or staging an update. */
  installing: boolean;
  /** The app update downloaded and verified, waiting for the user's
   * Restart or On quit: its version and archive URL. `checkForUpdate`
   * keeps the download of the same version only while the manifest's
   * archive (its SHA-256, which it does not report) stays the same, so
   * `restart` downloads again when it finds the download gone. */
  downloaded?: { version: string; url: string };
  /** The app version already staged with `installOnQuit`, so later checks
   * don't re-notify for the same update. */
  staged?: string;
  /** The latest update (package or app) a check found; a newer check
   * replaces it, so a Restart or On quit from an older message does
   * nothing. */
  offered: number;
}

const states = new WeakMap<Updater, State>();

function stateOf(updater: Updater): State {
  let state = states.get(updater);
  if (!state) states.set(updater, (state = { installing: false, offered: 0 }));
  return state;
}

/**
 * Checks for a newer build of the app's TSX (downloaded, it runs from the
 * next start; on the desktops the user can restart into it now), and when
 * there is none, for a newer app. An app this run can install is
 * downloaded first, as a package is, and offered once it is ready: Restart
 * installs and relaunches now, while On quit stages it quietly to swap
 * when the app exits without interrupting playback. One that cannot be
 * installed (or downloaded) here is offered as a link.
 * Resolves when done; never rejects. In development runs and on the web
 * it finds nothing.
 */
export function checkForUpdates(opts: UpdateOptions, updater: Updater = nshUpdater): Promise<void> {
  const state = stateOf(updater);
  if (state.checking || state.installing) return Promise.resolve();
  const check = runCheck(state, opts, updater).finally(() => {
    state.checking = undefined;
  });
  state.checking = check;
  return check;
}

async function runCheck(state: State, opts: UpdateOptions, updater: Updater): Promise<void> {
  try {
    const pkg = await updater.checkForPackage();
    if (pkg) {
      await updater.downloadPackage();
      // The package replaces what an earlier check offered: it runs from
      // the next start.
      const offer = ++state.offered;
      if (canRestart) {
        opts.notify(`nsh-music ${pkg.version} is ready.`, {
          label: "Restart",
          run: () => void runAction(state, offer, () => restartToPackage(updater)),
        });
      }
      return;
    }
    const app = await updater.checkForUpdate();
    // What this check found replaces what an earlier one offered.
    const offer = ++state.offered;
    // `checkForUpdate` drops its download when it finds no update it can
    // install.
    if (!app?.canInstall) state.downloaded = undefined;
    if (!app) {
      opts.onUpToDate?.();
      return;
    }
    if (state.staged === app.version) return;
    if (app.canInstall) {
      const same = state.downloaded?.version === app.version && state.downloaded.url === app.url;
      if (!same) {
        state.downloaded = undefined;
        try {
          await updater.downloadUpdate();
          state.downloaded = { version: app.version, url: app.url };
        } catch (error) {
          console.warn("update: download failed:", error);
        }
      }
      if (state.downloaded) {
        const readyMessage = app.shell ? `App shell ${app.version} is ready.` : `nsh-music ${app.version} is ready.`;
        opts.notify(
          readyMessage,
          {
            label: "Restart",
            run: () => void runAction(state, offer, () => installApp(state, app.version, opts, updater)),
          },
          {
            label: "On quit",
            run: () => void runAction(state, offer, () => stageApp(state, app.version, opts, updater)),
          },
        );
        return;
      }
    }
    opts.notify(`nsh-music ${app.version} is available.`, {
      label: "Download",
      run: () => void openUrl(app.url).catch(() => undefined),
    });
  } catch (error) {
    console.warn("update check failed:", error);
  }
}

/** Runs `go` for the action of `offer`, unless a later check offered
 * something else or an install/stage is already under way. */
async function runAction(state: State, offer: number, go: () => Promise<void>): Promise<void> {
  if (state.installing) return;
  // A check in progress holds the updater; it may also find something
  // newer, which it offers in its own message.
  await state.checking;
  if (offer !== state.offered || state.installing) return;
  state.installing = true;
  try {
    await go();
  } finally {
    state.installing = false;
  }
}

async function restartToPackage(updater: Updater): Promise<void> {
  await updater.restartToPackage().catch((e: unknown) => console.warn("update: restart failed:", e));
}

async function installApp(state: State, version: string, opts: UpdateOptions, updater: Updater): Promise<void> {
  try {
    // Unpacking and checking the new app takes a few seconds before it quits.
    opts.notify(`Updating to nsh-music ${version}…`);
    try {
      await updater.installAndRestart(); // Settles only on failure.
    } catch (error) {
      // A later check of the same version found a different archive, and
      // dropped this one: download that one now, then install it.
      if (!String(error).includes("nothing is downloaded")) throw error;
      state.downloaded = undefined;
      opts.notify(`Downloading nsh-music ${version}…`);
      await updater.downloadUpdate();
      opts.notify(`Updating to nsh-music ${version}…`);
      await updater.installAndRestart();
    }
  } catch (error) {
    console.warn("update: install failed:", error);
    // The next check downloads it again.
    state.downloaded = undefined;
    if (state.staged === version) state.staged = undefined;
    opts.notify("The update could not be installed.");
  }
}

async function stageApp(state: State, version: string, opts: UpdateOptions, updater: Updater): Promise<void> {
  try {
    await updater.installOnQuit();
    state.staged = version;
    opts.notify(`nsh-music ${version} will install on quit.`);
  } catch (error) {
    console.warn("update: stage failed:", error);
    state.downloaded = undefined;
    if (state.staged === version) state.staged = undefined;
    opts.notify("The update could not be installed.");
  }
}
