//! The browser the web player runs in: a Chromium of the system's, with a
//! profile of the app's own, started headless for playback or in a window
//! for signing in.
//!
//! Spotify's audio is encrypted (Widevine); Chromium keeps its Widevine
//! component in the profile and loads it at start. A new profile has none
//! until Chromium downloads it, so the app copies the component of a
//! Chromium or Chrome profile the user already has, when there is one.

use std::{
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

/// Where the app keeps the browser's profile (and with it the sign-in).
///
/// `SPOTISH_PROFILE` overrides it with an existing browser profile to reuse,
/// so a listener already signed in there needs no second sign-in. That
/// browser must be closed while Spotish runs (Chromium allows one instance
/// per profile). Spotish only reads the profile as that browser would; it
/// never decrypts cookies out of it.
pub fn profile_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("SPOTISH_PROFILE").filter(|p| !p.is_empty()) {
        return PathBuf::from(path);
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| home().map(|h| h.join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("spotish").join("browser")
}

/// Whether a chosen profile is already locked by a running browser.
pub fn profile_locked(profile: &Path) -> bool {
    // Chromium's `SingletonLock` is a symlink present while an instance holds
    // the profile.
    std::fs::symlink_metadata(profile.join("SingletonLock")).is_ok()
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// The Chromium-family browser to run: `SPOTISH_BROWSER`, else the first
/// one on `PATH`.
pub fn find_browser() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("SPOTISH_BROWSER") {
        return Some(PathBuf::from(path));
    }
    let names = [
        "chromium",
        "chromium-browser",
        "google-chrome-stable",
        "google-chrome",
        "brave",
        "brave-browser",
        "microsoft-edge-stable",
    ];
    #[cfg(target_os = "macos")]
    for app in [
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        "/Applications/Chromium.app/Contents/MacOS/Chromium",
    ] {
        if Path::new(app).exists() {
            return Some(app.into());
        }
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

const HINT: &str = "latest-component-updated-widevine-cdm";

/// Whether the profile has a Widevine component Chromium will load.
pub fn has_widevine(profile: &Path) -> bool {
    let hint = profile.join("WidevineCdm").join(HINT);
    std::fs::read_to_string(hint)
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|v| v.get("Path").and_then(|p| p.as_str()).map(PathBuf::from))
        .is_some_and(|dir| dir.join("manifest.json").exists())
}

/// Copies the Widevine component of another profile into `profile`; true
/// when it has one afterwards.
pub fn adopt_widevine(profile: &Path) -> bool {
    if has_widevine(profile) {
        return true;
    }
    let Some(home) = home() else { return false };
    let candidates = [
        home.join(".config/chromium/WidevineCdm"),
        home.join(".config/google-chrome/WidevineCdm"),
        home.join(".config/BraveSoftware/Brave-Browser/WidevineCdm"),
        home.join("Library/Application Support/Google/Chrome/WidevineCdm"),
    ];
    for source in candidates {
        // The newest version directory with a manifest.
        let Ok(entries) = std::fs::read_dir(&source) else { continue };
        let mut versions: Vec<PathBuf> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.join("manifest.json").exists())
            .collect();
        versions.sort();
        let Some(version) = versions.pop() else { continue };
        let target_root = profile.join("WidevineCdm");
        let target = target_root.join(version.file_name().unwrap());
        if copy_dir(&version, &target).is_err() {
            continue;
        }
        let hint = serde_json::json!({ "Path": target.to_string_lossy() }).to_string();
        if std::fs::write(target_root.join(HINT), hint).is_ok() {
            log::info!("using the Widevine component of {}", source.display());
            return true;
        }
    }
    false
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

/// How the browser shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// No window: the player plays, the app shows it.
    Headless,
    /// A window of its own, for signing in to Spotify.
    SignIn,
}

/// A running browser; it is stopped when dropped.
pub struct Browser {
    child: Child,
    pub port: u16,
}

pub const PLAYER_URL: &str = "https://open.spotify.com/";
pub const SIGN_IN_URL: &str = "https://accounts.spotify.com/login?continue=https%3A%2F%2Fopen.spotify.com%2F";

impl Browser {
    /// Starts `binary` on `profile` and waits for its DevTools port.
    pub fn start(binary: &Path, profile: &Path, mode: Mode) -> Result<Browser, String> {
        std::fs::create_dir_all(profile).map_err(|e| format!("{}: {e}", profile.display()))?;
        let port_file = profile.join("DevToolsActivePort");
        let _ = std::fs::remove_file(&port_file);
        let mut command = Command::new(binary);
        command
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--no-first-run")
            .arg("--disable-extensions")
            .arg("--no-default-browser-check")
            .arg("--disable-features=Translate,MediaRouter")
            .arg("--autoplay-policy=no-user-gesture-required")
            .arg("--password-store=basic");
        match mode {
            Mode::Headless => {
                command
                    .arg("--headless=new")
                    // The desktop layout, which has the player's whole bar.
                    .arg("--window-size=1280,900")
                    .arg("about:blank");
            }
            // An ordinary browser window for signing in: no bridge, no
            // automation flags (just `--remote-debugging-port`, which does not
            // set `navigator.webdriver`), so Google, Apple and Facebook sign-in
            // work. The DevTools port is used only to close it cleanly, so
            // Chromium flushes the new session's cookies to disk.
            Mode::SignIn => {
                command.arg("--window-size=520,760").arg(format!("--app={SIGN_IN_URL}"));
            }
        }
        command
            .arg("--remote-debugging-port=0")
            .arg("--remote-allow-origins=*")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(target_os = "linux")]
        unsafe {
            use std::os::unix::process::CommandExt;
            // The browser goes when the app does, however it ends.
            command.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
                Ok(())
            });
        }
        let child = command.spawn().map_err(|e| format!("{}: {e}", binary.display()))?;
        let mut browser = Browser { child, port: 0 };
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(text) = std::fs::read_to_string(&port_file)
                && let Some(port) = text.lines().next().and_then(|l| l.trim().parse::<u16>().ok())
            {
                browser.port = port;
                return Ok(browser);
            }
            if let Ok(Some(status)) = browser.child.try_wait() {
                return Err(format!("the browser quit at start ({status})"));
            }
            if Instant::now() > deadline {
                return Err("the browser did not open its DevTools port".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    /// Whether the browser is still running.
    pub fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Waits up to `timeout` for the browser to exit on its own (after a clean
    /// `Browser.close`), so its cookie writes land before the process goes.
    pub fn wait_for_exit(&mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if matches!(self.child.try_wait(), Ok(Some(_))) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// The WebSocket URL of its first page target.
    pub fn page_url(&self) -> Result<String, String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if let Ok(targets) = super::cdp::http_json(self.port, "/json/list")
                && let Some(url) = targets.as_array().and_then(|list| {
                    list.iter()
                        .find(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
                        .and_then(|t| t.get("webSocketDebuggerUrl").and_then(|v| v.as_str()))
                })
            {
                return Ok(url.to_owned());
            }
            if Instant::now() > deadline {
                return Err("the browser has no page".into());
            }
            std::thread::sleep(Duration::from_millis(150));
        }
    }

    /// The browser's user agent, as a desktop Chrome's (headless says so).
    pub fn user_agent(&self) -> Option<String> {
        let version = super::cdp::http_json(self.port, "/json/version").ok()?;
        let agent = version.get("User-Agent")?.as_str()?;
        Some(agent.replace("HeadlessChrome", "Chrome"))
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::kill(self.child.id() as i32, libc::SIGTERM);
        }
        #[cfg(not(unix))]
        let _ = self.child.kill();
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
