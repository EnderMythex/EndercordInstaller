// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::fs;
use std::path::PathBuf;

use crate::constants::{INSTALLER_DOWNLOAD_URL, INSTALLER_RELEASE_URL, TAG};
use crate::github;

fn asset_name() -> Option<String> {
    let gui = cfg!(feature = "gui");
    let arm = cfg!(target_arch = "aarch64");
    let name = match env::consts::OS {
        "windows" => format!(
            "EndercordInstaller{}{}.exe",
            if gui { "" } else { "Cli" },
            if arm { "-arm64" } else { "" }
        ),
        "linux" => format!(
            "EndercordInstaller{}-linux{}",
            if gui { "" } else { "Cli" },
            if arm { "-arm64" } else { "" }
        ),
        // A macOS app bundle can't replace itself in place.
        _ => return None,
    };
    Some(name)
}

fn old_exe() -> Option<PathBuf> {
    env::current_exe().ok().map(|p| p.with_extension("old"))
}

/// Removes the executable left behind by the previous self update.
pub fn cleanup() {
    if let Some(old) = old_exe() {
        let _ = fs::remove_file(old);
    }
}

/// Returns the newer tag if this is a release build and a newer installer exists.
pub fn check() -> Option<String> {
    let tag = TAG?;
    asset_name()?;
    let release = github::fetch_release(INSTALLER_RELEASE_URL).ok()?;
    (release.tag_name != tag).then_some(release.tag_name)
}

pub fn update() -> Result<(), String> {
    let name = asset_name().ok_or("Self updating isn't supported here. Download the new version from GitHub.")?;
    let exe = env::current_exe().map_err(|e| e.to_string())?;
    let new = exe.with_extension("new");

    crate::info!("Downloading the latest installer...");
    github::download(&format!("{INSTALLER_DOWNLOAD_URL}{name}"), &new)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&new, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
    }

    // A running executable can be renamed but not overwritten on Windows.
    let old = old_exe().ok_or("Couldn't find the current executable")?;
    fs::rename(&exe, &old).map_err(|e| e.to_string())?;
    if let Err(e) = fs::rename(&new, &exe) {
        let _ = fs::rename(&old, &exe);
        return Err(e.to_string());
    }

    crate::info!("Installer updated, restart it to use the new version");
    Ok(())
}
