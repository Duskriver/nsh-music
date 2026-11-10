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
  /** Shows `message` to the user, with a button running `action` when
   * given. */
  notify(message: string, action?: { label: string; run(): void }): void;
}

/** What `checkForUpdates` uses of `nsh/updater` (tests pass their own). */
export type Updater = Pick<
  typeof nshUpdater,
  "checkForPackage" | "checkForUpdate" | "downloadPackage" | "downloadUpdate" | "installAndRestart" | "restartToPackage"
>;

let checking = false;
/** The app version whose archive is downloaded and verified, waiting for
 * the user's Restart; `undefined` until one is. */
let downloaded: string | undefined;

/**
 * Checks for a newer build of the app's TSX (downloaded, it runs from the
 * next start; on the desktops the user can restart into it now), and when
 * there is none, for a newer app. An app this run can install is
 * downloaded first, as a package is, and offered once it is ready: Restart
 * then only installs it, a few seconds, rather than starting a download of
 * a minute or more with nothing on screen that quitting the app abandons.
 * One that cannot be installed (or downloaded) here is offered as a link.
 * Resolves when done; never rejects. In development runs and on the web
 * it finds nothing.
 */
export async function checkForUpdates(opts: UpdateOptions, updater: Updater = nshUpdater): Promise<void> {
  if (checking) return;
  checking = true;
  try {
    const pkg = await updater.checkForPackage();
    if (pkg) {
      await updater.downloadPackage();
      if (canRestart) {
        opts.notify(`nsh-music ${pkg.version} is ready.`, {
          label: "Restart",
          run: () => void updater.restartToPackage().catch((e: unknown) => console.warn("update: restart failed:", e)),
        });
      }
      return;
    }
    const app = await updater.checkForUpdate();
    if (!app) return;
    if (app.canInstall) {
      // The same version found again keeps its download (`checkForUpdate`):
      // a later check only offers it again.
      if (downloaded !== app.version) {
        try {
          await updater.downloadUpdate();
          downloaded = app.version;
        } catch (error) {
          console.warn("update: download failed:", error);
        }
      }
      if (downloaded === app.version) {
        opts.notify(`nsh-music ${app.version} is ready.`, {
          label: "Restart",
          run: () => void installApp(app.version, opts, updater),
        });
        return;
      }
    }
    opts.notify(`nsh-music ${app.version} is available.`, {
      label: "Download",
      run: () => void openUrl(app.url).catch(() => undefined),
    });
  } catch (error) {
    console.warn("update check failed:", error);
  } finally {
    checking = false;
  }
}

async function installApp(version: string, opts: UpdateOptions, updater: Updater): Promise<void> {
  // Unpacking and checking the new app takes a few seconds before it quits.
  opts.notify(`Updating to nsh-music ${version}…`);
  try {
    await updater.installAndRestart(); // Settles only on failure.
  } catch (error) {
    console.warn("update: install failed:", error);
    // The next check downloads it again (`checkForUpdate` may have
    // dropped this download for a different archive of the version).
    if (downloaded === version) downloaded = undefined;
    opts.notify("The update could not be installed.");
  }
}
