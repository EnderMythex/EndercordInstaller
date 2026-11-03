// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, Color32, RichText, Vec2};

use crate::constants::{GIT_HASH, TAG};
use crate::discord::{self, DiscordInstall};
use crate::github::{self, Release};
use crate::log::{Level, HISTORY};
use crate::{openasar, patcher, paths, selfupdate};

const LOGO: &[u8] = include_bytes!("../winres/icon.png");

const BG: Color32 = Color32::from_rgb(0x0b, 0x0d, 0x1a);
const CARD: Color32 = Color32::from_rgb(0x15, 0x1a, 0x33);
const ACCENT: Color32 = Color32::from_rgb(0x4a, 0x4f, 0x9e);
const LAVENDER: Color32 = Color32::from_rgb(0xb7, 0xa6, 0xe6);
const PEACH: Color32 = Color32::from_rgb(0xe2, 0xb4, 0x8f);
const TEXT: Color32 = Color32::from_rgb(0xec, 0xeb, 0xff);
const RED: Color32 = Color32::from_rgb(0xc0, 0x3a, 0x4a);

#[derive(Clone, Copy)]
enum Action {
    Install,
    Repair,
    Uninstall,
    InstallOpenAsar,
    UninstallOpenAsar,
    UpdateSelf,
}

#[derive(Default)]
struct Shared {
    release: Option<Result<Release, String>>,
    installer_update: Option<String>,
    installed_hash: Option<String>,
}

struct App {
    logo: egui::TextureHandle,
    installs: Vec<DiscordInstall>,
    openasar: Vec<bool>,
    selected: usize,
    shared: Arc<Mutex<Shared>>,
    busy: Arc<AtomicBool>,
    rescan: Arc<AtomicBool>,
}

fn load_icon() -> (Vec<u8>, u32, u32) {
    let img = image::load_from_memory(LOGO)
        .expect("bundled logo is a valid png")
        .to_rgba8();
    let (w, h) = img.dimensions();
    (img.into_raw(), w, h)
}

fn style(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = BG;
    visuals.window_fill = BG;
    visuals.extreme_bg_color = Color32::from_rgb(0x07, 0x08, 0x0f);
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = ACCENT;
    visuals.hyperlink_color = LAVENDER;
    visuals.widgets.inactive.weak_bg_fill = CARD;
    visuals.widgets.inactive.bg_fill = CARD;
    visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(0x2e, 0x3a, 0x6e);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(0x2e, 0x3a, 0x6e);
    visuals.widgets.active.weak_bg_fill = ACCENT;
    visuals.widgets.active.bg_fill = ACCENT;
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(14.0, 7.0);
    ctx.set_style(style);
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        style(&cc.egui_ctx);

        let (pixels, w, h) = load_icon();
        let logo = cc.egui_ctx.load_texture(
            "logo",
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels),
            Default::default(),
        );

        let shared = Arc::new(Mutex::new(Shared::default()));
        {
            let shared = shared.clone();
            let ctx = cc.egui_ctx.clone();
            thread::spawn(move || {
                let installed_hash = github::installed_hash();
                let release = if paths::get().dev_install {
                    None
                } else {
                    Some(github::latest())
                };
                let installer_update = selfupdate::check();
                *shared.lock().unwrap() = Shared {
                    release,
                    installer_update,
                    installed_hash,
                };
                ctx.request_repaint();
            });
        }

        let mut app = Self {
            logo,
            installs: vec![],
            openasar: vec![],
            selected: 0,
            shared,
            busy: Arc::new(AtomicBool::new(false)),
            rescan: Arc::new(AtomicBool::new(false)),
        };
        app.scan();
        app
    }

    fn scan(&mut self) {
        self.installs = discord::find_all();
        self.openasar = self.installs.iter().map(openasar::is_installed).collect();
        self.selected = self.selected.min(self.installs.len().saturating_sub(1));
    }

    fn run(&self, ctx: &egui::Context, action: Action) {
        let mut di = self.installs.get(self.selected).cloned();
        let release = match &self.shared.lock().unwrap().release {
            Some(Ok(r)) => Some(r.clone()),
            _ => None,
        };
        let (busy, rescan, ctx) = (self.busy.clone(), self.rescan.clone(), ctx.clone());
        busy.store(true, Ordering::SeqCst);

        thread::spawn(move || {
            let result = match (action, di.as_mut()) {
                (Action::UpdateSelf, _) => selfupdate::update(),
                (_, None) => Err("Select a Discord install first.".into()),
                (Action::Install, Some(di)) => {
                    github::install_latest(release.as_ref()).and_then(|_| patcher::patch(di))
                }
                (Action::Repair, Some(di)) => {
                    let p = paths::get();
                    if !p.dev_install && p.endercord.is_file() {
                        let _ = std::fs::remove_file(&p.endercord);
                    }
                    github::install_latest(release.as_ref()).and_then(|_| patcher::patch(di))
                }
                (Action::Uninstall, Some(di)) => patcher::unpatch(di),
                (Action::InstallOpenAsar, Some(di)) => openasar::install(di),
                (Action::UninstallOpenAsar, Some(di)) => openasar::uninstall(di),
            };
            match result {
                Ok(()) => crate::info!("Done! Restart Discord to see the changes."),
                Err(e) => crate::error!("{e}"),
            }
            rescan.store(true, Ordering::SeqCst);
            busy.store(false, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }

    fn header(&self, ui: &mut egui::Ui) {
        let shared = self.shared.lock().unwrap();
        ui.horizontal(|ui| {
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::splat(72.0)));
            ui.vertical(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new("EnderCord Installer").size(26.0).strong());
                let latest = match &shared.release {
                    _ if paths::get().dev_install => "local dev build".to_string(),
                    None => "checking...".to_string(),
                    Some(Ok(r)) => short(r.hash()).to_string(),
                    Some(Err(_)) => "unavailable".to_string(),
                };
                let installed = shared.installed_hash.as_deref().map(short).unwrap_or("none");
                ui.label(RichText::new(format!("Latest build: {latest}   Installed: {installed}")).color(LAVENDER));
            });
        });

        if let Some(Err(e)) = &shared.release {
            ui.colored_label(PEACH, e);
        }
        if paths::get().dev_install {
            ui.colored_label(
                PEACH,
                format!(
                    "Dev install: using your local build at {}",
                    paths::get().endercord.display()
                ),
            );
        }
    }

    fn installs(&mut self, ui: &mut egui::Ui) {
        ui.label(RichText::new("Discord installs").size(16.0).strong());
        egui::Frame::group(ui.style()).fill(CARD).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if self.installs.is_empty() {
                ui.label("No Discord install found. Install Discord first (snap isn't supported).");
            }
            for (i, di) in self.installs.iter().enumerate() {
                let mut text = RichText::new(di.label());
                if di.patched {
                    text = text.color(LAVENDER);
                }
                ui.radio_value(&mut self.selected, i, text);
            }
        });
    }

    fn buttons(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy.load(Ordering::SeqCst);
        let has_install = !self.installs.is_empty();
        let patched = self.installs.get(self.selected).is_some_and(|d| d.patched);
        let openasar = self.openasar.get(self.selected).copied().unwrap_or(false);
        let size = Vec2::new(150.0, 36.0);

        let button = |text: &str, fill: Color32| {
            egui::Button::new(RichText::new(text).strong())
                .fill(fill)
                .min_size(size)
        };

        ui.add_enabled_ui(!busy && has_install, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add(button(if patched { "Reinstall" } else { "Install" }, ACCENT))
                    .clicked()
                {
                    self.run(ui.ctx(), Action::Install);
                }
                if ui
                    .add(button("Repair", CARD))
                    .on_hover_text("Download the latest build and reinstall")
                    .clicked()
                {
                    self.run(ui.ctx(), Action::Repair);
                }
                if ui.add_enabled(patched, button("Uninstall", RED)).clicked() {
                    self.run(ui.ctx(), Action::Uninstall);
                }
            });
            ui.horizontal(|ui| {
                let (label, action) = if openasar {
                    ("Uninstall OpenAsar", Action::UninstallOpenAsar)
                } else {
                    ("Install OpenAsar", Action::InstallOpenAsar)
                };
                if ui
                    .add(button(label, CARD))
                    .on_hover_text("OpenAsar makes Discord start faster")
                    .clicked()
                {
                    self.run(ui.ctx(), action);
                }
                if ui.add(button("Refresh", CARD)).clicked() {
                    self.scan();
                }
            });
        });

        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Working, Discord will be closed if it's running...");
            });
        }
    }

    fn log(&self, ui: &mut egui::Ui) {
        egui::Frame::group(ui.style())
            .fill(Color32::from_rgb(0x07, 0x08, 0x0f))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        for (level, msg) in HISTORY.lock().unwrap().iter() {
                            let color = match level {
                                Level::Error => RED,
                                Level::Warn => PEACH,
                                _ => TEXT,
                            };
                            ui.label(RichText::new(msg).monospace().color(color));
                        }
                    });
            });
    }
}

fn short(hash: &str) -> &str {
    &hash[..hash.len().min(7)]
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if self.rescan.swap(false, Ordering::SeqCst) {
            self.scan();
            self.shared.lock().unwrap().installed_hash = github::installed_hash();
        }

        let update = self.shared.lock().unwrap().installer_update.clone();
        if let Some(tag) = update {
            egui::TopBottomPanel::top("update")
                .frame(egui::Frame::default().fill(ACCENT).inner_margin(8.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(format!("A new installer version ({tag}) is available."));
                        if ui
                            .add_enabled(!self.busy.load(Ordering::SeqCst), egui::Button::new("Update"))
                            .clicked()
                        {
                            self.run(ctx, Action::UpdateSelf);
                        }
                    });
                });
        }

        egui::TopBottomPanel::bottom("footer").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{} ({GIT_HASH})", TAG.unwrap_or("dev")))
                        .small()
                        .weak(),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.hyperlink_to("Support", crate::constants::SUPPORT_URL);
                    ui.hyperlink_to("Source", format!("https://github.com/{}", crate::constants::REPO));
                });
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(4.0);
            self.header(ui);
            ui.add_space(6.0);
            self.installs(ui);
            self.buttons(ui);
            self.log(ui);
        });
    }
}

pub fn run() {
    let (pixels, w, h) = load_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("EnderCord Installer")
            .with_inner_size([620.0, 560.0])
            .with_min_inner_size([520.0, 460.0])
            .with_icon(egui::IconData {
                rgba: pixels,
                width: w,
                height: h,
            }),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "EnderCord Installer",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    ) {
        crate::error!("Failed to open the installer window: {e}");
        std::process::exit(1);
    }
}
