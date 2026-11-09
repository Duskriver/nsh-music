//! The platform data directory for the native music session.

use std::{
    fs,
    path::{Path, PathBuf},
};

pub const SESSION_FILE: &str = "netease-session.json";

/// The app's data directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    /// Where the native login session is stored.
    pub data: PathBuf,
}

impl Paths {
    /// Resolves the app's data directory and moves any earlier NetEase session into it.
    pub fn platform() -> Self {
        for key in ["NSH_MUSIC_DATA_DIR", "SPOTISH_DATA_DIR"] {
            if let Some(dir) = std::env::var_os(key).filter(|d| !d.is_empty()) {
                return Self { data: dir.into() };
            }
        }

        let (current, previous) = if let Some(base) = data_root() {
            (base.join(DATA_DIR), base.join(LEGACY_DATA_DIR))
        } else {
            (PathBuf::from(".nsh-music"), PathBuf::from(".spotish"))
        };
        Self { data: migrate_session(current, &previous) }
    }
}

#[cfg(target_os = "macos")]
const DATA_DIR: &str = "io.github.duskriver.nsh-music";
#[cfg(target_os = "macos")]
const LEGACY_DATA_DIR: &str = "dev.huacnlee.spotish";

#[cfg(target_os = "windows")]
const LEGACY_DATA_DIR: &str = "Spotish";
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const LEGACY_DATA_DIR: &str = "spotish";
#[cfg(not(target_os = "macos"))]
const DATA_DIR: &str = "nsh-music";

fn data_root() -> Option<PathBuf> {
    if cfg!(target_os = "windows") { dirs::data_local_dir() } else { dirs::data_dir() }
}

fn migrate_session(current: PathBuf, previous: &Path) -> PathBuf {
    let old_session = previous.join(SESSION_FILE);
    let new_session = current.join(SESSION_FILE);
    let marker = current.join(".netease-session-migrated");
    if !old_session.is_file() || marker.is_file() {
        return current;
    }
    if new_session.exists() {
        // The current login takes precedence, including after the user logs out.
        if let Err(error) = fs::File::create(marker) {
            log::warn!("could not record the NetEase session migration: {error}");
        }
        return current;
    }
    match fs::create_dir_all(&current).and_then(|()| fs::rename(&old_session, &new_session)) {
        Ok(()) => current,
        Err(_) if new_session.is_file() => current,
        Err(error) => {
            log::warn!("could not move the existing NetEase session: {error}");
            previous.to_path_buf()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_only_the_netease_session_and_never_reuses_it_after_logout() {
        let root = std::env::temp_dir().join(format!("nsh-music-migration-{}", std::process::id()));
        let previous = root.join("previous");
        let current = root.join("current");
        fs::create_dir_all(&previous).unwrap();
        fs::write(previous.join(SESSION_FILE), "example session").unwrap();
        fs::write(previous.join("unrelated"), "keep").unwrap();

        assert_eq!(migrate_session(current.clone(), &previous), current);
        assert_eq!(fs::read_to_string(current.join(SESSION_FILE)).unwrap(), "example session");
        assert!(previous.join("unrelated").is_file());
        assert!(!previous.join(SESSION_FILE).exists());

        fs::remove_file(current.join(SESSION_FILE)).unwrap();
        assert_eq!(migrate_session(current.clone(), &previous), current);
        assert!(!current.join(SESSION_FILE).exists());

        // If both directories contain a session, keep the current one even across logout.
        fs::write(previous.join(SESSION_FILE), "legacy session").unwrap();
        fs::write(current.join(SESSION_FILE), "current session").unwrap();
        assert_eq!(migrate_session(current.clone(), &previous), current);
        assert_eq!(fs::read_to_string(current.join(SESSION_FILE)).unwrap(), "current session");
        fs::remove_file(current.join(SESSION_FILE)).unwrap();
        assert_eq!(migrate_session(current.clone(), &previous), current);
        assert!(!current.join(SESSION_FILE).exists());

        fs::remove_dir_all(root).unwrap();
    }
}
