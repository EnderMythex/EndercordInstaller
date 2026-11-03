// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(not(target_os = "macos"))]
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct DiscordInstall {
    /// The base install path shown to the user.
    pub path: PathBuf,
    pub branch: String,
    /// `resources/app` of the version we patch. Its parent holds app.asar.
    pub app_path: PathBuf,
    pub patched: bool,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub flatpak: bool,
    /// Distro packages running Discord on the system Electron have app.asar directly in `path`.
    pub system_electron: bool,
}

impl DiscordInstall {
    pub fn asar_dir(&self) -> PathBuf {
        if self.system_electron {
            self.path.clone()
        } else {
            self.app_path.parent().unwrap_or(&self.path).to_path_buf()
        }
    }

    pub fn label(&self) -> String {
        let mut branch = self.branch.clone();
        if let Some(first) = branch.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        format!(
            "{branch} - {}{}",
            self.path.display(),
            if self.patched { " [EnderCord installed]" } else { "" }
        )
    }
}

pub fn get_branch(name: &str) -> String {
    let name = name.to_lowercase();
    ["canary", "development", "ptb"]
        .into_iter()
        .find(|b| name.ends_with(b))
        .unwrap_or("stable")
        .to_string()
}

#[cfg(not(target_os = "macos"))]
fn parse_app_version(name: &str) -> Option<Vec<u32>> {
    name.strip_prefix("app-")?.split('.').map(|p| p.parse().ok()).collect()
}

/// Squirrel style installs (Windows, and the new Linux updater) keep each version in an `app-x.y.z` folder.
/// Returns the newest one that has an asar.
#[cfg(not(target_os = "macos"))]
fn latest_app_dir(p: &Path, preferred: Option<String>) -> Option<(PathBuf, bool)> {
    let try_dir = |name: &str| -> Option<(Vec<u32>, PathBuf, bool)> {
        let ver = parse_app_version(name)?;
        let resources = p.join(name).join("resources");
        let patched = resources.join("_app.asar").exists();
        if !patched && !resources.join("app.asar").exists() {
            return None;
        }
        Some((ver, resources.join("app"), patched))
    };

    if let Some(found) = preferred.as_deref().and_then(try_dir) {
        return Some((found.1, found.2));
    }

    fs::read_dir(p)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| try_dir(&e.file_name().to_string_lossy()))
        .max_by(|a, b| a.0.cmp(&b.0))
        .map(|(_, app, patched)| (app, patched))
}

#[cfg(target_os = "windows")]
const WINDOWS_NAMES: &[(&str, &str)] = &[
    ("stable", "Discord"),
    ("ptb", "DiscordPTB"),
    ("canary", "DiscordCanary"),
    ("dev", "DiscordDevelopment"),
];

#[cfg(target_os = "windows")]
pub fn parse(p: &Path, branch: Option<&str>) -> Option<DiscordInstall> {
    let (app_path, patched) = latest_app_dir(p, None)?;
    Some(DiscordInstall {
        path: p.to_path_buf(),
        branch: branch
            .map(String::from)
            .unwrap_or_else(|| get_branch(&p.to_string_lossy())),
        app_path,
        patched,
        flatpak: false,
        system_electron: false,
    })
}

#[cfg(target_os = "windows")]
pub fn find_all() -> Vec<DiscordInstall> {
    let Some(local) = std::env::var_os("LOCALAPPDATA") else {
        crate::error!("%LOCALAPPDATA% is empty, can't look for Discord");
        return vec![];
    };
    WINDOWS_NAMES
        .iter()
        .filter_map(|(branch, dir)| parse(&Path::new(&local).join(dir), Some(branch)))
        .collect()
}

/// Closing Discord from outside skips its normal shutdown, where it saves the login token back,
/// so a forced close logs you out. Used to ask the user to quit Discord themselves first.
#[cfg(target_os = "windows")]
pub fn is_running(di: &DiscordInstall) -> bool {
    use std::os::windows::process::CommandExt;
    let Some((_, name)) = WINDOWS_NAMES.iter().find(|(b, _)| *b == di.branch) else {
        return false;
    };
    let exe = format!("{name}.exe");
    std::process::Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {exe}"), "/NH"])
        .creation_flags(0x0800_0000)
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&exe))
}

/// Only Windows locks app.asar while Discord runs, so other platforms never need to close it.
#[cfg(not(target_os = "windows"))]
pub fn is_running(_: &DiscordInstall) -> bool {
    false
}

/// Discord keeps app.asar open, so it has to be closed before we can rename it.
#[cfg(target_os = "windows")]
pub fn prepare(di: &DiscordInstall) {
    use std::os::windows::process::CommandExt;
    let Some((_, name)) = WINDOWS_NAMES.iter().find(|(b, _)| *b == di.branch) else {
        return;
    };
    crate::debug!("Closing {name}.exe");
    let status = std::process::Command::new("taskkill")
        .args(["/F", "/T", "/IM", &format!("{name}.exe")])
        .creation_flags(0x0800_0000)
        .output();
    if let Ok(out) = status {
        if out.status.success() {
            std::thread::sleep(std::time::Duration::from_millis(1500));
        }
    }
}

#[cfg(target_os = "macos")]
const MACOS_NAMES: &[(&str, &str)] = &[
    ("stable", "Discord.app"),
    ("ptb", "Discord PTB.app"),
    ("canary", "Discord Canary.app"),
    ("dev", "Discord Development.app"),
];

#[cfg(target_os = "macos")]
pub fn parse(p: &Path, branch: Option<&str>) -> Option<DiscordInstall> {
    let resources = p.join("Contents/Resources");
    let patched = resources.join("_app.asar").exists();
    if !patched && !resources.join("app.asar").exists() {
        return None;
    }
    let name = p.to_string_lossy();
    Some(DiscordInstall {
        path: p.to_path_buf(),
        branch: branch
            .map(String::from)
            .unwrap_or_else(|| get_branch(name.trim_end_matches(".app"))),
        app_path: resources.join("app"),
        patched,
        flatpak: false,
        system_electron: false,
    })
}

#[cfg(target_os = "macos")]
pub fn find_all() -> Vec<DiscordInstall> {
    let mut bases = vec![PathBuf::from("/Applications")];
    if let Some(home) = dirs::home_dir() {
        bases.push(home.join("Applications"));
    }
    MACOS_NAMES
        .iter()
        .flat_map(|(branch, dir)| bases.iter().filter_map(move |b| parse(&b.join(dir), Some(branch))))
        .collect()
}

#[cfg(target_os = "macos")]
pub fn prepare(_: &DiscordInstall) {}

#[cfg(target_os = "linux")]
fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_default()
}

/// Installs managed by Discord's own updater in ~/.config/discord or the flatpak config folder.
#[cfg(target_os = "linux")]
fn parse_new(p: &Path, branch: Option<&str>, flatpak: bool) -> Option<DiscordInstall> {
    let preferred = fs::read_link(p.join("Discord"))
        .ok()
        .and_then(|t| t.parent()?.file_name().map(|n| n.to_string_lossy().into_owned()));
    let (app_path, patched) = latest_app_dir(p, preferred)?;
    Some(DiscordInstall {
        path: p.to_path_buf(),
        branch: branch
            .map(String::from)
            .unwrap_or_else(|| get_branch(&p.to_string_lossy())),
        app_path,
        patched,
        flatpak,
        system_electron: false,
    })
}

#[cfg(target_os = "linux")]
fn parse_classic(p: &Path) -> Option<DiscordInstall> {
    let name = p.file_name()?.to_string_lossy().into_owned();
    let mut p = p.to_path_buf();

    let s = p.to_string_lossy();
    let flatpak = s.contains("/flatpak/") && !s.contains("/current/active/files/");
    if flatpak {
        let mut discord = name.strip_prefix("com.discordapp.")?.to_lowercase();
        if discord != "discord" && discord.len() > 7 {
            discord.insert(7, '-');
        }
        p = p.join("current/active/files").join(discord);
    }

    let resources = p.join("resources");
    let (patched, system_electron) = if resources.exists() {
        (resources.join("_app.asar").exists(), false)
    } else if p.join("app.asar").exists() {
        (p.join("_app.asar.unpacked").exists(), true)
    } else {
        return None;
    };

    Some(DiscordInstall {
        path: p,
        branch: get_branch(&name),
        app_path: resources.join("app"),
        patched,
        flatpak,
        system_electron,
    })
}

#[cfg(target_os = "linux")]
pub fn parse(p: &Path, branch: Option<&str>) -> Option<DiscordInstall> {
    parse_new(p, branch, p.to_string_lossy().contains("com.discordapp")).or_else(|| parse_classic(p))
}

#[cfg(target_os = "linux")]
pub fn find_all() -> Vec<DiscordInstall> {
    use std::collections::HashSet;

    let home = home();
    let dirs = [
        PathBuf::from("/usr/share"),
        PathBuf::from("/usr/lib"),
        PathBuf::from("/usr/lib64"),
        PathBuf::from("/opt"),
        home.join("Applications"),
        home.join("AppImages"),
        home.join(".local/share"),
        home.join(".local/bin"),
        home.join(".dvm"),
        home.join(".config"),
        home.join(".var/app"),
        PathBuf::from("/var/lib/flatpak/app"),
        home.join(".local/share/flatpak/app"),
    ];

    let mut seen = HashSet::new();
    let mut found = vec![];

    for dir in dirs {
        let Ok(dir) = fs::canonicalize(&dir) else { continue };
        if !seen.insert(dir.clone()) {
            continue;
        }
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !entry.path().is_dir() || !crate::constants::LINUX_DISCORD_NAMES.contains(&name.as_str()) {
                continue;
            }
            if let Some(d) = parse_classic(&entry.path()) {
                crate::debug!("Found Discord install at {}", d.path.display());
                found.push(d);
            }
        }
    }

    for name in ["discord", "discordcanary", "discordptb"] {
        let dir = home.join(".config").join(name);
        if let Some(d) = dir
            .exists()
            .then(|| parse_new(&dir, Some(&get_branch(name)), false))
            .flatten()
        {
            found.push(d);
        }
    }

    for name in ["Discord", "DiscordCanary", "DiscordPTB"] {
        let dir = home
            .join(".var/app")
            .join(format!("com.discordapp.{name}"))
            .join("config/discord");
        if let Some(d) = dir
            .exists()
            .then(|| parse_new(&dir, Some(&get_branch(name)), true))
            .flatten()
        {
            found.push(d);
        }
    }

    found
}

#[cfg(target_os = "linux")]
pub fn prepare(_: &DiscordInstall) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn branches() {
        assert_eq!(get_branch("DiscordCanary"), "canary");
        assert_eq!(get_branch("discord-ptb"), "ptb");
        assert_eq!(get_branch("Discord"), "stable");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn picks_newest_version() {
        let dir = std::env::temp_dir().join(format!("endercord-test-{}", std::process::id()));
        for v in ["app-1.0.9", "app-1.0.10", "app-0.9.99"] {
            fs::create_dir_all(dir.join(v).join("resources")).unwrap();
            fs::write(dir.join(v).join("resources/app.asar"), b"x").unwrap();
        }
        let (app, patched) = latest_app_dir(&dir, None).unwrap();
        assert_eq!(app, dir.join("app-1.0.10/resources/app"));
        assert!(!patched);
        fs::remove_dir_all(dir).unwrap();
    }
}
