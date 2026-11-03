// SPDX-License-Identifier: GPL-3.0-or-later
// EnderCord Installer, based on the Vencord Installer by Vendicated and contributors

#![cfg_attr(all(feature = "gui", not(debug_assertions)), windows_subsystem = "windows")]

mod asar;
#[cfg_attr(feature = "gui", allow(dead_code))]
mod cli;
mod constants;
mod discord;
mod github;
#[cfg(feature = "gui")]
mod gui;
mod log;
mod openasar;
mod patcher;
mod paths;
mod selfupdate;

fn main() {
    if std::env::args().any(|a| a == "--debug") {
        log::DEBUG.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    #[cfg(target_os = "linux")]
    if let Err(e) = paths::handle_root() {
        error!("{e}");
        std::process::exit(1);
    }

    selfupdate::cleanup();

    #[cfg(feature = "gui")]
    gui::run();

    #[cfg(not(feature = "gui"))]
    cli::run();
}
