// The system theme's mapping onto nsh-music's tokens, with two real Omarchy
// palettes (./fixtures): pure functions, no app needed.
import { describe, expect, test } from "nsh/test";

import { adventure, catppuccinLatte } from "./fixtures/palettes";
import { contrast, mapPalette, mix, parseHex } from "./palette";
import { needsPlaybackUpdate } from "./media";
import { trayIcon, trayTitle } from "./desktop";
import { themeFor } from "./theme";
import { checkForUpdates, type Updater } from "./updater";
import { colorScheme, setColorScheme, setTheme, token } from "nsh";
import { isAvailable as canRestart } from "nsh/desktop";

/** Every token theme.json defines a color for. */
const COLOR_TOKENS = [
  "background", "foreground", "card", "card-foreground", "popover", "popover-foreground",
  "primary", "primary-foreground", "secondary", "secondary-foreground", "muted", "muted-foreground",
  "accent", "accent-foreground", "destructive", "border", "input", "ring",
  "sidebar", "sidebar-foreground", "sidebar-accent", "sidebar-accent-foreground", "sidebar-border",
  "shell", "player", "control", "control-foreground", "row-hover",
  "chart-1", "chart-2", "chart-3", "chart-4", "chart-5",
];

describe("color helpers", () => {
  test("parse and mix hex colors", () => {
    expect(parseHex("#fff")).toEqual([255, 255, 255]);
    expect(parseHex("#11223344")).toEqual([17, 34, 51]);
    expect(parseHex("rgb(1,2,3)")).toBeNull();
    expect(mix("#000000", "#ffffff", 0.5)).toBe("#808080");
    expect(contrast("#000000", "#ffffff")).toBeGreaterThan(20.9);
  });
});

describe("mapPalette", () => {
  for (const [name, palette] of [["adventure", adventure], ["catppuccin-latte", catppuccinLatte]] as const) {
    test(`${name}: fills every color token with #rrggbb`, () => {
      const t = mapPalette(palette)!;
      for (const key of COLOR_TOKENS) expect(t[key] ?? `missing ${key}`).toMatch(/^#[0-9a-f]{6}$/);
    });

    test(`${name}: text reads on its surfaces`, () => {
      const t = mapPalette(palette)!;
      expect(contrast(t.foreground, t.background)).toBeGreaterThanOrEqual(7);
      expect(contrast(t["muted-foreground"], t.background)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(t["primary-foreground"], t.primary)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(t["accent-foreground"], t.accent)).toBeGreaterThanOrEqual(4.5);
      expect(contrast(t.primary, t.background)).toBeGreaterThanOrEqual(3);
      expect(contrast(t["control-foreground"], t.control)).toBeGreaterThanOrEqual(7);
    });
  }

  test("adventure (dark): takes the theme's colors; the frame recedes", () => {
    const t = mapPalette(adventure)!;
    expect(t.background).toBe("#040404");
    expect(t.foreground).toBe("#feffff");
    expect(t.primary).toBe("#5da602");
    expect(t.ring).toBe("#5da602");
    expect(t.destructive).toBe("#d84a33");
    // Raised surfaces are lighter than the background, the shell darker.
    expect(contrast(t.secondary, "#000000")).toBeGreaterThan(contrast(t.background, "#000000"));
    expect(contrast(t.shell, "#000000")).toBeLessThanOrEqual(contrast(t.background, "#000000"));
  });

  test("catppuccin-latte (light): dark text on light surfaces", () => {
    const t = mapPalette(catppuccinLatte)!;
    expect(t.background).toBe("#eff1f5");
    expect(t.primary).toBe("#1e66f5");
    // Raised and hovered surfaces step toward the text: darker in a light theme.
    expect(contrast(t["row-hover"], "#ffffff")).toBeGreaterThan(contrast(t.background, "#ffffff"));
    expect(contrast(t.foreground, "#ffffff")).toBeGreaterThan(7);
  });

  test("a palette without background or foreground maps to null", () => {
    expect(mapPalette({ mode: "dark", colors: { accent: "#ff0000" } })).toBeNull();
  });

  test("an accent too dim to see on the background is raised to 3:1", () => {
    const t = mapPalette({ mode: "dark", colors: { background: "#0a0708", foreground: "#bfb8b6", accent: "#3a0d12" } })!;
    expect(contrast(t.primary, t.background)).toBeGreaterThanOrEqual(3);
  });
});

describe("themeFor", () => {
  test("NSH takes the mapped theme, keeping theme.json's lengths", () => {
    const radius = token("radius");
    const theme = themeFor(adventure, { radius: `${radius}px`, "font-mono": token("font-mono") })!;
    try {
      setTheme(theme);
      setColorScheme(adventure.mode);
      expect(colorScheme()).toBe("dark");
      expect(token("background")).toMatch(/^#040404/);
      expect(token("primary")).toMatch(/^#5da602/);
      expect(token("radius")).toBe(radius);
    } finally {
      setTheme(null);
      setColorScheme("system");
    }
  });
});

describe("media controls", () => {
  test("report play/pause and seeks, not every tick", () => {
    const last = { playing: true, positionMs: 10_000, at: 0 };
    expect(needsPlaybackUpdate(null, last)).toBe(true);
    expect(needsPlaybackUpdate(last, { playing: true, positionMs: 11_000, at: 1_000 })).toBe(false);
    expect(needsPlaybackUpdate(last, { playing: false, positionMs: 11_000, at: 1_000 })).toBe(true);
    expect(needsPlaybackUpdate(last, { playing: true, positionMs: 60_000, at: 1_000 })).toBe(true);
  });
});

describe("tray", () => {
  test("titles and icon", () => {
    expect(trayTitle("  ")).toBe("Not playing");
    expect(trayTitle("x".repeat(80))).toHaveLength(60);
    expect(decodeURIComponent(trayIcon("#ffeeddff"))).toContain('fill="#ffeedd"');
  });
});

describe("updates", () => {
  test("a development run finds nothing and says nothing", async () => {
    const said: string[] = [];
    await checkForUpdates({ notify: (message) => said.push(message) });
    expect(said).toEqual([]);
  });

  /** A release in the fake source's manifest. */
  interface Release {
    version: string;
    sha256: string;
  }

  interface Source {
    release: Release | null;
    check?: () => Promise<Release | null>;
    pkg: string | null;
  }

  /** A fake `nsh/updater` that keeps its download as NSH's runtime does
   * (`checkForUpdate` keeps it for the same version and SHA-256 only, one
   * operation at a time). `steps` gets each call and message in order;
   * `source.release` is what the manifest offers, `source.check` replaces
   * a check, `source.pkg` is the OTA package the manifest offers. */
  function fakeUpdater(
    release: Release | null,
    steps: string[],
    fail: { download?: boolean; install?: boolean; stage?: boolean; shell?: boolean } = {},
  ): Updater & { source: Source } {
    const source: Source = { release, pkg: null };
    let pending: Release | null = null;
    let archive: string | null = null;
    let nextPackage: string | null = null;
    let busy = false;
    async function operation<T>(work: () => Promise<T>): Promise<T> {
      if (busy) throw new Error("an update operation is already in progress");
      busy = true;
      try {
        return await work();
      } finally {
        busy = false;
      }
    }
    return {
      source,
      checkForPackage: () =>
        operation(async () => {
          const pkg = source.pkg;
          return pkg && { version: pkg, build: 2, notes: null, size: null };
        }),
      // The package stays on disk until the app restarts into it.
      downloadPackage: () =>
        operation(async () => {
          steps.push("download package");
          nextPackage = source.pkg;
        }),
      restartToPackage: () =>
        operation(async () => {
          if (!nextPackage) throw new Error("nothing is downloaded: call downloadPackage first");
          steps.push(`restart into ${nextPackage}`);
          return new Promise<void>(() => undefined);
        }),
      checkForUpdate: () =>
        operation(async () => {
          steps.push("check");
          const found = source.check ? await source.check() : source.release;
          if (!found) {
            pending = archive = null;
            return null;
          }
          if (pending?.version !== found.version || pending.sha256 !== found.sha256) {
            pending = found;
            archive = null;
          }
          const url = `https://example.invalid/nsh-music-${found.version}.tar.gz`;
          return { version: found.version, notes: null, url, canInstall: true, shell: fail.shell ?? false };
        }),
      downloadUpdate: () =>
        operation(async () => {
          steps.push("download");
          if (fail.download) throw new Error("HTTP 503");
          archive = pending && `${pending.version}/${pending.sha256}`;
        }),
      installAndRestart: () =>
        operation(async () => {
          if (!archive) throw new Error("nothing is downloaded: call downloadUpdate first");
          steps.push(`install ${archive}`);
          if (fail.install) throw new Error("its code signature is not valid");
          return new Promise<void>(() => undefined); // Restarts: never settles.
        }),
      installOnQuit: () =>
        operation(async () => {
          steps.push("stage");
          if (fail.stage) throw new Error("its code signature is not valid");
        }),
    } as unknown as Updater & { source: Source };
  }

  /** A `notify` that logs into `steps` and keeps the last actions. */
  function recorder(steps: string[], includeSecondary = false) {
    const last: { action?: { label: string; run(): void }; secondary?: { label: string; run(): void } } = {};
    return {
      last,
      opts: {
        notify(
          message: string,
          action?: { label: string; run(): void },
          secondary?: { label: string; run(): void },
        ) {
          const labels = [
            includeSecondary && secondary ? `[${secondary.label}]` : "",
            action ? `[${action.label}]` : "",
          ].filter(Boolean);
          steps.push(labels.length ? `${message} ${labels.join(" ")}` : message);
          last.action = action;
          last.secondary = secondary;
        },
      },
    };
  }

  const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

  test("an app update is downloaded before it is offered, and Restart only installs it", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater({ version: "9.0.0", sha256: "a" }, steps);
    const { last, opts } = recorder(steps);
    await checkForUpdates(opts, updater);
    expect(steps).toEqual(["check", "download", "nsh-music 9.0.0 is ready. [Restart]"]);
    last.action?.run();
    await settle();
    expect(steps.slice(3)).toEqual(["Updating to nsh-music 9.0.0…", "install 9.0.0/a"]);
  });

  test("a later check offers the downloaded version again without downloading it again", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater({ version: "9.1.0", sha256: "a" }, steps);
    const { last, opts } = recorder(steps);
    await checkForUpdates(opts, updater);
    await checkForUpdates(opts, updater);
    expect(steps.filter((s) => s === "download")).toHaveLength(1);
    expect(steps.at(-1)).toBe("nsh-music 9.1.0 is ready. [Restart]");
    last.action?.run();
    await settle();
    expect(steps.at(-1)).toBe("install 9.1.0/a");
  });

  test("a new archive of the same version is downloaded on Restart, and installed", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater({ version: "9.2.0", sha256: "a" }, steps);
    const { last, opts } = recorder(steps);
    await checkForUpdates(opts, updater);
    // The release's archive is replaced: the runtime drops the download,
    // which the app cannot see (`checkForUpdate` reports no SHA-256).
    updater.source.release = { version: "9.2.0", sha256: "b" };
    await checkForUpdates(opts, updater);
    last.action?.run();
    await settle();
    expect(steps.slice(3)).toEqual([
      "check",
      "nsh-music 9.2.0 is ready. [Restart]",
      "Updating to nsh-music 9.2.0…",
      "Downloading nsh-music 9.2.0…",
      "download",
      "Updating to nsh-music 9.2.0…",
      "install 9.2.0/b",
    ]);
  });

  test("Restart from an older offer does nothing once a later check offers a newer version", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater({ version: "9.3.0", sha256: "a" }, steps);
    const { last, opts } = recorder(steps);
    await checkForUpdates(opts, updater);
    const older = last.action;
    updater.source.release = { version: "9.4.0", sha256: "a" };
    await checkForUpdates(opts, updater);
    older?.run();
    await settle();
    expect(steps.some((s) => s.startsWith("install") || s.startsWith("Updating"))).toBe(false);
    last.action?.run();
    await settle();
    expect(steps.slice(-2)).toEqual(["Updating to nsh-music 9.4.0…", "install 9.4.0/a"]);
  });

  test("Restart during a check that finds the release gone does nothing", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater({ version: "9.6.0", sha256: "a" }, steps);
    const { last, opts } = recorder(steps);
    await checkForUpdates(opts, updater);
    let resolve = (_: Release | null) => undefined as void;
    updater.source.check = () => new Promise((done) => (resolve = done));
    const checking = checkForUpdates(opts, updater);
    last.action?.run();
    await settle();
    resolve(null);
    await checking;
    await settle();
    expect(steps.slice(3)).toEqual(["check"]);
  });

  test("a download that fails offers the release's page instead", async () => {
    const steps: string[] = [];
    const { last, opts } = recorder(steps);
    await checkForUpdates(opts, fakeUpdater({ version: "9.7.0", sha256: "a" }, steps, { download: true }));
    expect(steps).toEqual(["check", "download", "nsh-music 9.7.0 is available. [Download]"]);
    expect(last.action?.label).toBe("Download");
  });

  test("an install that fails says so, and the next check downloads again", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater({ version: "9.8.0", sha256: "a" }, steps, { install: true });
    const { last, opts } = recorder(steps);
    await checkForUpdates(opts, updater);
    last.action?.run();
    await settle();
    expect(steps.slice(3)).toEqual(["Updating to nsh-music 9.8.0…", "install 9.8.0/a", "The update could not be installed."]);
    await checkForUpdates(opts, updater);
    expect(steps.slice(6)).toEqual(["check", "download", "nsh-music 9.8.0 is ready. [Restart]"]);
  });

  test("Restart from an older app offer does nothing once a later check finds a package", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater({ version: "9.9.0", sha256: "a" }, steps);
    const { opts, last } = recorder(steps);
    await checkForUpdates(opts, updater);
    const older = last.action;
    updater.source.pkg = "9.9.1";
    await checkForUpdates(opts, updater);
    older?.run();
    await settle();
    expect(steps.some((s) => s.startsWith("install") || s.startsWith("Updating"))).toBe(false);
    expect(steps.slice(3, 4)).toEqual(["download package"]);
  });

  // No package Restart is offered where the app cannot restart: these
  // report as skipped there instead of passing without running.
  const restartTest = canRestart ? test : test.skip;

  restartTest("Restart from an older package offer does nothing once a later check offers an app update", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater(null, steps);
    updater.source.pkg = "9.10.0";
    const { opts, last } = recorder(steps);
    await checkForUpdates(opts, updater);
    expect(steps).toEqual(["download package", "nsh-music 9.10.0 is ready. [Restart]"]);
    const older = last.action;
    updater.source.pkg = null;
    updater.source.release = { version: "9.11.0", sha256: "a" };
    await checkForUpdates(opts, updater);
    older?.run();
    await settle();
    expect(steps.some((s) => s.startsWith("restart into"))).toBe(false);
    last.action?.run();
    await settle();
    expect(steps.slice(-2)).toEqual(["Updating to nsh-music 9.11.0…", "install 9.11.0/a"]);
  });

  restartTest("Restart into a package during a check waits for it, and restarts when it fails", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater(null, steps);
    updater.source.pkg = "9.12.0";
    const { opts, last } = recorder(steps);
    await checkForUpdates(opts, updater);
    updater.source.pkg = null;
    let fail = (_: Error) => undefined as void;
    updater.source.check = () => new Promise((_, reject) => (fail = reject));
    const checking = checkForUpdates(opts, updater);
    last.action?.run();
    await settle();
    // Nothing collides with the check, which holds the updater.
    expect(steps.slice(2)).toEqual(["check"]);
    fail(new Error("offline"));
    await checking;
    await settle();
    expect(steps.slice(3)).toEqual(["restart into 9.12.0"]);
  });

  test("On quit stages the update without restarting and quiets later checks", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater({ version: "9.13.0", sha256: "a" }, steps);
    const { last, opts } = recorder(steps, true);
    await checkForUpdates(opts, updater);
    expect(steps).toEqual(["check", "download", "nsh-music 9.13.0 is ready. [On quit] [Restart]"]);
    last.secondary?.run();
    await settle();
    expect(steps.slice(3)).toEqual(["stage", "nsh-music 9.13.0 will install on quit."]);
    // Already staged for quit: a later check does not re-download or notify again.
    await checkForUpdates(opts, updater);
    expect(steps).toEqual([
      "check",
      "download",
      "nsh-music 9.13.0 is ready. [On quit] [Restart]",
      "stage",
      "nsh-music 9.13.0 will install on quit.",
      "check",
    ]);
  });

  test("a stage that fails says so and lets the next check try again", async () => {
    const steps: string[] = [];
    const updater = fakeUpdater({ version: "9.14.0", sha256: "a" }, steps, { stage: true });
    const { last, opts } = recorder(steps, true);
    await checkForUpdates(opts, updater);
    last.secondary?.run();
    await settle();
    expect(steps.slice(3)).toEqual(["stage", "The update could not be installed."]);
    await checkForUpdates(opts, fakeUpdater({ version: "9.14.0", sha256: "a" }, steps));
    expect(steps.slice(5)).toEqual(["check", "download", "nsh-music 9.14.0 is ready. [On quit] [Restart]"]);
  });

  test("shell-only updates name the app shell", async () => {
    const steps: string[] = [];
    const { opts } = recorder(steps, true);
    await checkForUpdates(opts, fakeUpdater({ version: "9.15.0", sha256: "a" }, steps, { shell: true }));
    expect(steps).toEqual(["check", "download", "App shell 9.15.0 is ready. [On quit] [Restart]"]);
  });

  test("onUpToDate runs only when neither package nor app update is available", async () => {
    const steps: string[] = [];
    let upToDateCount = 0;
    const { opts } = recorder(steps);
    await checkForUpdates({ ...opts, onUpToDate: () => upToDateCount++ }, fakeUpdater(null, steps));
    expect(upToDateCount).toBe(1);

    await checkForUpdates(
      { ...opts, onUpToDate: () => upToDateCount++ },
      fakeUpdater({ version: "9.16.0", sha256: "a" }, steps),
    );
    expect(upToDateCount).toBe(1);
  });
});
