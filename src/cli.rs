// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;
use std::process::exit;

use clap::Parser;
use dialoguer::{Input, Select};

use crate::constants::{GIT_HASH, REPO, SUPPORT_URL, TAG};
use crate::discord::{self, DiscordInstall};
use crate::{github, openasar, patcher, paths, selfupdate};

#[derive(Parser)]
#[command(
    name = "EndercordInstaller",
    about = "Install EnderCord on your Discord desktop app.",
    disable_version_flag = true
)]
struct Args {
    /// Install EnderCord
    #[arg(long)]
    install: bool,
    /// Download the latest EnderCord build and reinstall it
    #[arg(long)]
    repair: bool,
    /// Uninstall EnderCord
    #[arg(long)]
    uninstall: bool,
    /// Install OpenAsar
    #[arg(long)]
    install_openasar: bool,
    /// Uninstall OpenAsar
    #[arg(long)]
    uninstall_openasar: bool,
    /// Update the installer itself
    #[arg(long)]
    update_self: bool,
    /// The Discord install folder to modify
    #[arg(long, conflicts_with = "branch")]
    location: Option<PathBuf>,
    /// The Discord branch to modify
    #[arg(long, value_parser = ["auto", "stable", "ptb", "canary"])]
    branch: Option<String>,
    /// Print debug info
    #[arg(long)]
    debug: bool,
    /// Print the installer version
    #[arg(long)]
    version: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Action {
    Install,
    Repair,
    Uninstall,
    InstallOpenAsar,
    UninstallOpenAsar,
}

fn done(ok: bool, interactive: bool) -> ! {
    if ok {
        println!("\u{2714} Success!");
    } else {
        println!("\u{274c} Failed! If this keeps happening, see {SUPPORT_URL}");
    }
    if cfg!(windows) && interactive {
        let _ = Input::<String>::new()
            .with_prompt("Press Enter to exit")
            .allow_empty(true)
            .interact_text();
    }
    exit(if ok { 0 } else { 1 })
}

fn die(msg: &str, interactive: bool) -> ! {
    crate::error!("{msg}");
    done(false, interactive)
}

fn pick_discord(action: &str, args: &Args, interactive: bool) -> DiscordInstall {
    let installs = discord::find_all();

    if let Some(branch) = args.branch.as_deref() {
        let wanted: &[&str] = if branch == "auto" {
            &["stable", "canary", "ptb"]
        } else {
            &[branch]
        };
        return wanted
            .iter()
            .find_map(|b| installs.iter().find(|d| d.branch == *b).cloned())
            .unwrap_or_else(|| {
                die(
                    "No matching Discord install found. Make sure Discord is installed (snap isn't supported).",
                    interactive,
                )
            });
    }

    if let Some(location) = &args.location {
        return discord::parse(location, None).unwrap_or_else(|| {
            die(
                &format!("{} is not a valid Discord install.", location.display()),
                interactive,
            )
        });
    }

    let mut items: Vec<String> = installs.iter().map(DiscordInstall::label).collect();
    items.push("Custom location".into());

    let choice = Select::new()
        .with_prompt(format!("Select the Discord install to {action}"))
        .items(&items)
        .default(0)
        .interact_opt()
        .ok()
        .flatten()
        .unwrap_or_else(|| exit(0));

    if let Some(d) = installs.get(choice) {
        return d.clone();
    }

    loop {
        let custom: String = Input::new()
            .with_prompt("Discord location")
            .interact_text()
            .unwrap_or_else(|_| exit(0));
        match discord::parse(&PathBuf::from(custom.trim()), None) {
            Some(d) => return d,
            None => crate::error!("That's not a valid Discord install."),
        }
    }
}

pub fn run() {
    let args = Args::parse();
    crate::log::DEBUG.store(args.debug, std::sync::atomic::Ordering::Relaxed);

    if args.version {
        println!("EndercordInstaller Cli {} ({GIT_HASH})", TAG.unwrap_or("dev"));
        println!("Copyright (C) 2026 Vendicated, Equicord, PawsomeVencord and EnderCord contributors");
        println!("License GPLv3+: GNU GPL version 3 or later <https://gnu.org/licenses/gpl.html>.");
        println!("Source: https://github.com/{REPO}");
        return;
    }

    if args.update_self {
        done(selfupdate::update().map_err(|e| crate::error!("{e}")).is_ok(), false);
    }

    let flags = [
        (args.install, Action::Install),
        (args.repair, Action::Repair),
        (args.uninstall, Action::Uninstall),
        (args.install_openasar, Action::InstallOpenAsar),
        (args.uninstall_openasar, Action::UninstallOpenAsar),
    ];
    let interactive = !flags.iter().any(|(set, _)| *set);

    let action = match flags.iter().find(|(set, _)| *set) {
        Some((_, action)) => *action,
        None => {
            if let Some(tag) = selfupdate::check() {
                crate::warn!("A new installer version ({tag}) is available. Pick \"Update the installer\" or run with --update-self.");
            }
            let choices = [
                "Install EnderCord",
                "Repair EnderCord",
                "Uninstall EnderCord",
                "Install OpenAsar",
                "Uninstall OpenAsar",
                "Update the installer",
                "Quit",
            ];
            let choice = Select::new()
                .with_prompt("What would you like to do?")
                .items(&choices)
                .default(0)
                .interact_opt()
                .ok()
                .flatten()
                .unwrap_or(6);
            match choice {
                0 => Action::Install,
                1 => Action::Repair,
                2 => Action::Uninstall,
                3 => Action::InstallOpenAsar,
                4 => Action::UninstallOpenAsar,
                5 => done(selfupdate::update().map_err(|e| crate::error!("{e}")).is_ok(), true),
                _ => return,
            }
        }
    };

    let verb = match action {
        Action::Install | Action::InstallOpenAsar | Action::UninstallOpenAsar => "patch",
        Action::Repair => "repair",
        Action::Uninstall => "unpatch",
    };
    let mut di = pick_discord(verb, &args, interactive);

    let result = match action {
        Action::Install => patcher::patch(&mut di),
        Action::Repair => {
            let p = paths::get();
            if !p.dev_install && p.endercord.is_file() {
                let _ = std::fs::remove_file(&p.endercord);
            }
            github::install_latest(None).and_then(|_| patcher::patch(&mut di))
        }
        Action::Uninstall => patcher::unpatch(&mut di),
        Action::InstallOpenAsar => openasar::install(&di),
        Action::UninstallOpenAsar => openasar::uninstall(&di),
    };

    if let Err(e) = &result {
        crate::error!("{e}");
    }
    done(result.is_ok(), interactive)
}
