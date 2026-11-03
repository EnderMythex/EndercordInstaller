// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::asar;
use crate::discord::{self, DiscordInstall};
use crate::paths;

/// Turns the "file in use" error Windows gives while Discord runs into something readable.
fn friendly(err: io::Error) -> String {
    if cfg!(windows) && err.raw_os_error() == Some(32) {
        "Discord's files are used by another process. Close Discord completely and try again.".into()
    } else {
        err.to_string()
    }
}

/// Renames files and undoes every rename if a later step fails, so an install is never left half patched.
struct Renames(Vec<(PathBuf, PathBuf)>);

impl Renames {
    fn rename(&mut self, from: &Path, to: &Path) -> Result<(), String> {
        crate::debug!("Renaming {} to {}", from.display(), to.display());
        fs::rename(from, to).map_err(friendly)?;
        self.0.push((from.to_path_buf(), to.to_path_buf()));
        Ok(())
    }

    fn rollback(self) {
        for (from, to) in self.0.into_iter().rev() {
            if let Err(e) = fs::rename(&to, &from) {
                crate::error!("Failed to undo a partial change, Discord may need a reinstall: {e}");
            }
        }
    }
}

fn patch_dir(dir: &Path, system_electron: bool) -> Result<(), String> {
    let app_asar = dir.join("app.asar");
    let orig_asar = dir.join("_app.asar");
    let mut renames = Renames(vec![]);

    let result = (|| {
        renames.rename(&app_asar, &orig_asar)?;
        if system_electron {
            renames.rename(&dir.join("app.asar.unpacked"), &dir.join("_app.asar.unpacked"))?;
        }
        crate::debug!("Writing loader to {}", app_asar.display());
        asar::write_loader(&app_asar, &paths::get().endercord).map_err(friendly)
    })();

    if result.is_err() {
        crate::error!("Patching failed, undoing changes");
        renames.rollback();
    }
    result
}

fn unpatch_dir(dir: &Path, system_electron: bool) -> Result<(), String> {
    let app_asar = dir.join("app.asar");
    let orig_asar = dir.join("_app.asar");
    let tmp_asar = dir.join("app.asar.tmp");

    // Discord updated itself while patched: app.asar is a real Discord asar again and _app.asar is stale.
    if !asar::is_loader(&app_asar).map_err(friendly)? {
        crate::warn!("Discord was updated while patched, removing the leftover _app.asar");
        fs::remove_file(&orig_asar).map_err(friendly)?;
        if system_electron {
            fs::remove_dir_all(dir.join("_app.asar.unpacked")).map_err(friendly)?;
        }
        return Ok(());
    }

    let mut renames = Renames(vec![]);
    let result = (|| {
        renames.rename(&app_asar, &tmp_asar)?;
        renames.rename(&orig_asar, &app_asar)?;
        if system_electron {
            renames.rename(&dir.join("_app.asar.unpacked"), &dir.join("app.asar.unpacked"))?;
        }
        Ok(())
    })();

    match &result {
        Ok(()) => {
            let _ = fs::remove_file(&tmp_asar);
        }
        Err(_) => {
            crate::error!("Unpatching failed, undoing changes");
            renames.rollback();
        }
    }
    result
}

pub fn unpatch(di: &mut DiscordInstall) -> Result<(), String> {
    crate::info!("Uninstalling EnderCord from {}...", di.path.display());
    discord::prepare(di);
    unpatch_dir(&di.asar_dir(), di.system_electron)?;
    di.patched = false;
    crate::info!("Successfully uninstalled from {}", di.path.display());
    Ok(())
}

pub fn patch(di: &mut DiscordInstall) -> Result<(), String> {
    crate::info!("Installing EnderCord on {}...", di.path.display());
    crate::github::install_latest(None)?;
    discord::prepare(di);

    if di.patched {
        crate::info!("Already patched, removing the old patch first...");
        unpatch(di)?;
    }

    patch_dir(&di.asar_dir(), di.system_electron)?;
    di.patched = true;

    #[cfg(target_os = "linux")]
    if di.flatpak {
        grant_flatpak_access(di)?;
    }

    crate::info!("Successfully installed on {}", di.path.display());
    Ok(())
}

/// Flatpak Discord is sandboxed and can't read our files unless we add an override.
#[cfg(target_os = "linux")]
fn grant_flatpak_access(di: &DiscordInstall) -> Result<(), String> {
    use std::process::Command;

    let path = di.path.to_string_lossy();
    let name = path
        .split('/')
        .find(|e| e.starts_with("com.discordapp"))
        .ok_or("Couldn't figure out the Flatpak app id")?
        .to_string();

    let system = path.starts_with("/var");
    let target = paths::get().endercord.to_string_lossy().into_owned();
    let mut args: Vec<String> = vec![];
    if !system {
        args.push("--user".into());
    }
    args.extend(["override".into(), name, format!("--filesystem={target}")]);

    crate::debug!("Running flatpak {}", args.join(" "));
    let sudo_user = std::env::var("SUDO_USER").unwrap_or_default();
    let status = if !system && !sudo_user.is_empty() && unsafe { libc::getuid() } == 0 {
        Command::new("sudo")
            .arg("-u")
            .arg(&sudo_user)
            .arg("flatpak")
            .args(&args)
            .status()
    } else {
        Command::new("flatpak").args(&args).status()
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(s) => Err(format!("Failed to give Discord's Flatpak access to {target} ({s})")),
        Err(e) => Err(format!("Failed to run flatpak: {e}")),
    }
}
