// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub struct Paths {
    /// Data folder shared with the client (settings, themes...).
    pub base_dir: PathBuf,
    /// What the loader app.asar requires: the downloaded asar, or a dist folder for dev installs.
    pub endercord: PathBuf,
    /// Set by `pnpm inject` in the EnderCord repo, which points us at a local build.
    pub dev_install: bool,
}

static PATHS: OnceLock<Paths> = OnceLock::new();

pub fn get() -> &'static Paths {
    PATHS.get_or_init(|| {
        let base_dir = if let Some(dir) = env::var_os("EQUICORD_USER_DATA_DIR") {
            PathBuf::from(dir)
        } else if let Some(dir) = env::var_os("DISCORD_USER_DATA_DIR") {
            PathBuf::from(dir).join("..").join("EquicordData")
        } else {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("Equicord")
        };

        let endercord = match env::var_os("EQUICORD_DIRECTORY") {
            Some(dir) => PathBuf::from(dir),
            None => base_dir.join("endercord.asar"),
        };

        Paths {
            base_dir,
            endercord,
            dev_install: env::var("EQUICORD_DEV_INSTALL").as_deref() == Ok("1"),
        }
    })
}

pub fn ensure_base_dir() -> std::io::Result<()> {
    let dir = &get().base_dir;
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
        fix_ownership(dir);
    }
    Ok(())
}

/// When run through sudo, HOME points at root. Point it back at the real user before anything reads it.
#[cfg(target_os = "linux")]
pub fn handle_root() -> Result<(), String> {
    let mut sudo_user = env::var("SUDO_USER").unwrap_or_default();
    if sudo_user.is_empty() {
        sudo_user = env::var("DOAS_USER").unwrap_or_default();
        if !sudo_user.is_empty() {
            env::set_var("SUDO_USER", &sudo_user);
        }
    }

    if sudo_user == "root" {
        return Err("Don't run the installer as the root user. Run it as your normal user, with sudo or doas.".into());
    }

    if sudo_user.is_empty() {
        if unsafe { libc::getuid() } == 0 {
            return Err("The installer was run as root without SUDO_USER or DOAS_USER. Run it as your normal user, with sudo or doas.".into());
        }
        return Ok(());
    }

    if let Some(home) = user_home(&sudo_user) {
        crate::debug!(
            "Running through sudo, using the home of {sudo_user}: {}",
            home.display()
        );
        env::set_var("HOME", home);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn user_ids(user: &str) -> Option<(libc::uid_t, libc::gid_t, PathBuf)> {
    use std::ffi::{CStr, CString};
    let name = CString::new(user).ok()?;
    let pw = unsafe { libc::getpwnam(name.as_ptr()) };
    if pw.is_null() {
        return None;
    }
    let pw = unsafe { &*pw };
    let home = unsafe { CStr::from_ptr(pw.pw_dir) }.to_string_lossy().into_owned();
    Some((pw.pw_uid, pw.pw_gid, PathBuf::from(home)))
}

#[cfg(target_os = "linux")]
fn user_home(user: &str) -> Option<PathBuf> {
    user_ids(user).map(|(_, _, home)| home)
}

/// Files we create as root must belong to the real user, or Discord can't read them.
#[cfg(target_os = "linux")]
pub fn fix_ownership(path: &Path) {
    if unsafe { libc::geteuid() } != 0 {
        return;
    }
    let Ok(user) = env::var("SUDO_USER") else { return };
    let Some((uid, gid, _)) = user_ids(&user) else { return };

    fn walk(path: &Path, uid: u32, gid: u32) {
        if let Err(e) = std::os::unix::fs::lchown(path, Some(uid), Some(gid)) {
            crate::warn!("Failed to fix ownership of {}: {e}", path.display());
        }
        if path.is_dir() {
            for entry in std::fs::read_dir(path).into_iter().flatten().flatten() {
                walk(&entry.path(), uid, gid);
            }
        }
    }
    walk(path, uid, gid);
}

#[cfg(not(target_os = "linux"))]
pub fn fix_ownership(_: &Path) {}
