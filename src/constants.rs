// SPDX-License-Identifier: GPL-3.0-or-later
// EnderCord Installer, based on the Vencord Installer by Vendicated and contributors

pub const NAME: &str = "EnderCord";
pub const REPO: &str = "EnderMythex/Endercord";
pub const INSTALLER_REPO: &str = "EnderMythex/EndercordInstaller";

pub const RELEASE_URL: &str = "https://api.github.com/repos/EnderMythex/Endercord/releases/latest";
pub const INSTALLER_RELEASE_URL: &str = "https://api.github.com/repos/EnderMythex/EndercordInstaller/releases/latest";
pub const INSTALLER_DOWNLOAD_URL: &str = "https://github.com/EnderMythex/EndercordInstaller/releases/latest/download/";
pub const SUPPORT_URL: &str = "https://github.com/EnderMythex/EndercordInstaller/issues";
pub const OPENASAR_URL: &str = "https://github.com/GooseMod/OpenAsar/releases/download/nightly/app.asar";

/// Set by CI. Builds without a tag are dev builds and never self update.
pub const TAG: Option<&str> = option_env!("ENDERCORD_INSTALLER_TAG");
pub const GIT_HASH: &str = match option_env!("ENDERCORD_INSTALLER_HASH") {
    Some(h) => h,
    None => "dev",
};

/// Written into the loader app.asar. Must stay byte identical to the Vencord/Equicord installers
/// so installs patched by them are recognised as patched.
pub const PACKAGE_JSON: &str = "{\n\t\"name\": \"discord\",\n\t\"main\": \"index.js\"\n}";

pub fn user_agent() -> String {
    format!("EndercordInstaller/{GIT_HASH} (https://github.com/{INSTALLER_REPO})")
}

#[cfg(target_os = "linux")]
pub const LINUX_DISCORD_NAMES: &[&str] = &[
    "Discord",
    "DiscordPTB",
    "DiscordCanary",
    "DiscordDevelopment",
    "discord",
    "discordptb",
    "discordcanary",
    "discorddevelopment",
    "discord-ptb",
    "discord-canary",
    "discord-development",
    "com.discordapp.Discord",
    "com.discordapp.DiscordPTB",
    "com.discordapp.DiscordCanary",
    "com.discordapp.DiscordDevelopment",
];
