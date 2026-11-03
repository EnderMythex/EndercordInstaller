// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::time::Duration;

use serde::Deserialize;

use crate::constants::{user_agent, NAME, RELEASE_URL};
use crate::paths;

#[derive(Deserialize, Clone, Debug)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Release {
    #[serde(default)]
    pub name: String,
    pub tag_name: String,
    pub assets: Vec<Asset>,
}

impl Release {
    /// The EnderCord build workflow names the release "EnderCord <commit hash>".
    pub fn hash(&self) -> &str {
        self.name.rsplit(' ').next().unwrap_or_default()
    }

    pub fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }
}

pub fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .user_agent(&user_agent())
        .build()
}

pub fn fetch_release(url: &str) -> Result<Release, String> {
    crate::debug!("Fetching {url}");
    agent()
        .get(url)
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => "No release has been published on GitHub yet.".to_string(),
            ureq::Error::Status(403 | 429, _) => "GitHub is rate limiting you. Try again in a few minutes.".to_string(),
            e => format!("Couldn't reach GitHub: {e}"),
        })?
        .into_json()
        .map_err(|e| format!("Failed to read release info: {e}"))
}

pub fn latest() -> Result<Release, String> {
    fetch_release(RELEASE_URL)
}

/// Hash of the installed build, read from the banner the build script puts at the top of the bundle.
pub fn installed_hash() -> Option<String> {
    let target = &paths::get().endercord;
    let file = if target.is_dir() {
        target.join("patcher.js")
    } else {
        target.clone()
    };

    let mut head = vec![0; 4096];
    let n = fs::File::open(file).ok()?.read(&mut head).ok()?;
    let head = String::from_utf8_lossy(&head[..n]);

    let marker = format!("// {NAME} ");
    let start = head.find(&marker)? + marker.len();
    let hash: String = head[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric())
        .collect();
    (!hash.is_empty()).then_some(hash)
}

/// Downloads to a temporary file next to the destination, then swaps it in.
pub fn download(url: &str, dest: &Path) -> Result<(), String> {
    crate::debug!("Downloading {url}");
    let res = agent().get(url).call().map_err(|e| format!("Download failed: {e}"))?;
    let expected: Option<u64> = res.header("Content-Length").and_then(|l| l.parse().ok());

    let tmp = dest.with_extension("download");
    let mut file = fs::File::create(&tmp).map_err(|e| format!("Failed to create {}: {e}", tmp.display()))?;
    let written = io::copy(&mut res.into_reader(), &mut file).map_err(|e| format!("Download interrupted: {e}"))?;
    drop(file);

    if expected.is_some_and(|len| len != written) {
        let _ = fs::remove_file(&tmp);
        return Err(format!(
            "Download was cut off ({written} of {} bytes). Try again.",
            expected.unwrap()
        ));
    }

    fs::rename(&tmp, dest).map_err(|e| format!("Failed to move the download into place: {e}"))?;
    paths::fix_ownership(dest);
    Ok(())
}

/// Makes sure the latest EnderCord build is on disk. Dev installs use the local build instead.
pub fn install_latest(release: Option<&Release>) -> Result<(), String> {
    let p = paths::get();
    if p.dev_install {
        crate::debug!("Dev install, using the local build at {}", p.endercord.display());
        return Ok(());
    }

    let owned;
    let release = match release {
        Some(r) => r,
        None => {
            owned = latest()?;
            &owned
        }
    };

    if p.endercord.is_file() && installed_hash().as_deref() == Some(release.hash()) {
        crate::debug!("EnderCord files are already up to date");
        return Ok(());
    }

    let asset = release
        .asset("desktop.asar")
        .ok_or("The latest EnderCord release has no desktop.asar. Wait for the build to finish and try again.")?;

    paths::ensure_base_dir().map_err(|e| format!("Failed to create {}: {e}", p.base_dir.display()))?;
    crate::info!("Downloading the latest {NAME} build...");
    download(&asset.browser_download_url, &p.endercord)
}
