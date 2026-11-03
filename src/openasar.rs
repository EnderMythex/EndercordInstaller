// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::PathBuf;

use crate::constants::OPENASAR_URL;
use crate::discord::{self, DiscordInstall};
use crate::github;

/// The real Discord asar: `_app.asar` when EnderCord is installed, `app.asar` otherwise.
fn discord_asar(di: &DiscordInstall) -> Result<PathBuf, String> {
    let dir = di.asar_dir();
    ["_app.asar", "app.asar"]
        .into_iter()
        .map(|f| dir.join(f))
        .find(|p| p.is_file())
        .ok_or_else(|| format!("No asar file found in {}", dir.display()))
}

pub fn is_installed(di: &DiscordInstall) -> bool {
    let Ok(bytes) = discord_asar(di).and_then(|p| fs::read(p).map_err(|e| e.to_string())) else {
        return false;
    };
    bytes.windows(8).any(|w| w == b"OpenAsar")
}

pub fn install(di: &DiscordInstall) -> Result<(), String> {
    if is_installed(di) {
        return Err("OpenAsar is already installed.".into());
    }
    discord::prepare(di);

    let asar = discord_asar(di)?;
    let backup = di.asar_dir().join("app.asar.backup");
    fs::rename(&asar, &backup).map_err(|e| e.to_string())?;

    crate::info!("Downloading OpenAsar...");
    if let Err(e) = github::download(OPENASAR_URL, &asar) {
        if let Err(e) = fs::rename(&backup, &asar) {
            crate::error!("Failed to restore Discord's app.asar, Discord may need a reinstall: {e}");
        }
        return Err(e);
    }
    crate::info!("OpenAsar installed");
    Ok(())
}

pub fn uninstall(di: &DiscordInstall) -> Result<(), String> {
    if !is_installed(di) {
        return Err("OpenAsar is not installed.".into());
    }
    discord::prepare(di);

    let dir = di.asar_dir();
    let backup = ["app.asar.backup", "app.asar.original"]
        .into_iter()
        .map(|f| dir.join(f))
        .find(|p| p.exists())
        .ok_or("No OpenAsar backup found. Reinstall Discord to remove OpenAsar.")?;

    fs::rename(backup, discord_asar(di)?).map_err(|e| e.to_string())?;
    crate::info!("OpenAsar uninstalled");
    Ok(())
}
