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

const BG: Color32 = Color32::from_rgb(0x06, 0x06, 0x07);
const TEXT: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf7);
const MUTED: Color32 = Color32::from_rgb(0x86, 0x86, 0x8b);
const GLASS: Color32 = Color32::from_rgba_premultiplied(14, 14, 15, 14);
const GLASS_HOVER: Color32 = Color32::from_rgba_premultiplied(26, 26, 28, 26);
const LINE: Color32 = Color32::from_rgba_premultiplied(20, 20, 20, 20);
const RED: Color32 = Color32::from_rgb(0xff, 0x6b, 0x6b);
const AMBER: Color32 = Color32::from_rgb(0xff, 0xc8, 0x6b);

#[derive(Clone, Copy, PartialEq)]
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
    particles: Vec<Particle>,
    installs: Vec<DiscordInstall>,
    openasar: Vec<bool>,
    selected: usize,
    shared: Arc<Mutex<Shared>>,
    busy: Arc<AtomicBool>,
    rescan: Arc<AtomicBool>,
    /// Set once an action succeeded, to offer opening Discord.
    finished: Arc<AtomicBool>,
    /// Action waiting for the user to quit Discord, so it isn't force closed.
    confirm: Option<Action>,
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
    visuals.extreme_bg_color = BG;
    visuals.override_text_color = Some(TEXT);
    visuals.selection.bg_fill = Color32::from_white_alpha(56);
    visuals.selection.stroke = egui::Stroke::new(1.0_f32, TEXT);
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    visuals.hyperlink_color = MUTED;
    for w in [
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.noninteractive,
    ] {
        w.corner_radius = egui::CornerRadius::same(12);
        w.bg_stroke = egui::Stroke::new(1.0_f32, LINE);
    }
    visuals.widgets.inactive.weak_bg_fill = GLASS;
    visuals.widgets.inactive.bg_fill = GLASS;
    visuals.widgets.hovered.weak_bg_fill = GLASS_HOVER;
    visuals.widgets.hovered.bg_fill = GLASS_HOVER;
    visuals.widgets.active.weak_bg_fill = GLASS_HOVER;
    visuals.widgets.active.bg_fill = GLASS_HOVER;
    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(8.0, 8.0);
    style.spacing.button_padding = Vec2::new(16.0, 8.0);
    style.interaction.selectable_labels = false;
    ctx.set_style(style);
}

struct Particle {
    angle: f32,
    offset: f32,
    size: f32,
    phase: f32,
    core: bool,
}

/// Same particle ring as the logo and Discord's startup screen: a crisp core line plus dust.
fn make_particles() -> Vec<Particle> {
    let mut seed = 7u32;
    let mut rnd = move || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (seed >> 8) as f32 / (1u32 << 24) as f32
    };
    (0..700)
        .map(|i| {
            let core = i % 3 == 0;
            let angle = rnd() * std::f32::consts::TAU;
            let gauss = (rnd() + rnd() + rnd() - 1.5) / 1.5;
            let dust = 0.5 + 0.5 * (angle + 1.2).sin();
            let side = if rnd() < 0.5 { -0.6 } else { 1.0 };
            Particle {
                angle,
                offset: if core {
                    gauss * 0.6
                } else {
                    gauss.abs() * (1.5 + 11.0 * dust) * side
                },
                size: if core { 0.8 } else { 0.4 + rnd() * 0.6 },
                phase: rnd() * std::f32::consts::TAU,
                core,
            }
        })
        .collect()
}

fn paint_ring(ui: &egui::Ui, particles: &[Particle], center: egui::Pos2, radius: f32) {
    let painter = ui.painter();
    let t = ui.input(|i| i.time) as f32;
    let sweep = t * 0.5;
    for p in particles {
        let angle = p.angle + t * 0.12;
        let r = radius + p.offset + (t + p.phase).sin() * if p.core { 0.15 } else { 0.8 };
        let light = (0.5 + 0.5 * (angle - sweep).cos()).powf(if p.core { 8.0 } else { 3.0 });
        let fade = if p.core {
            1.0
        } else {
            (1.0 - p.offset.abs() / 13.0).max(0.0)
        };
        let alpha = if p.core {
            0.06 + light
        } else {
            (0.04 + 0.6 * light) * fade
        };
        painter.circle_filled(
            center + Vec2::angled(angle) * r,
            p.size,
            Color32::from_white_alpha((alpha.min(1.0) * 255.0) as u8),
        );
    }
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
}

fn glass_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).size(13.0)).min_size(Vec2::new(0.0, 34.0))
}

impl App {
    fn new(cc: &eframe::CreationContext) -> Self {
        style(&cc.egui_ctx);

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
            particles: make_particles(),
            installs: vec![],
            openasar: vec![],
            selected: 0,
            shared,
            busy: Arc::new(AtomicBool::new(false)),
            rescan: Arc::new(AtomicBool::new(false)),
            finished: Arc::new(AtomicBool::new(false)),
            confirm: None,
        };
        app.scan();
        app
    }

    fn scan(&mut self) {
        self.installs = discord::find_all();
        self.openasar = self.installs.iter().map(openasar::is_installed).collect();
        self.selected = self.selected.min(self.installs.len().saturating_sub(1));
    }

    fn request(&mut self, ctx: &egui::Context, action: Action) {
        if self.installs.get(self.selected).is_some_and(discord::is_running) {
            self.confirm = Some(action);
        } else {
            self.run(ctx, action);
        }
    }

    fn run(&self, ctx: &egui::Context, action: Action) {
        let mut di = self.installs.get(self.selected).cloned();
        let release = match &self.shared.lock().unwrap().release {
            Some(Ok(r)) => Some(r.clone()),
            _ => None,
        };
        let (busy, rescan, finished, ctx) = (
            self.busy.clone(),
            self.rescan.clone(),
            self.finished.clone(),
            ctx.clone(),
        );
        busy.store(true, Ordering::SeqCst);
        finished.store(false, Ordering::SeqCst);

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
                Ok(()) if action == Action::UpdateSelf => crate::info!("Done! Restart the installer."),
                Ok(()) => {
                    finished.store(true, Ordering::SeqCst);
                    match di.as_ref().map(discord::launch) {
                        Some(Ok(())) => crate::info!("Done! Starting Discord..."),
                        Some(Err(e)) => crate::warn!("Done! {e}"),
                        None => {}
                    }
                }
                Err(e) => crate::error!("{e}"),
            }
            rescan.store(true, Ordering::SeqCst);
            busy.store(false, Ordering::SeqCst);
            ctx.request_repaint();
        });
    }

    fn header(&self, ui: &mut egui::Ui) {
        let shared = self.shared.lock().unwrap();
        ui.vertical_centered(|ui| {
            ui.add_space(18.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(120.0), egui::Sense::hover());
            paint_ring(ui, &self.particles, rect.center(), 44.0);
            ui.add_space(14.0);
            ui.label(RichText::new("EnderCord").size(30.0).color(TEXT));
            ui.label(RichText::new("I N S T A L L E R").size(10.0).color(MUTED));
            ui.add_space(4.0);

            let latest = match &shared.release {
                _ if paths::get().dev_install => "local build".to_string(),
                None => "checking".to_string(),
                Some(Ok(r)) => short(r.hash()).to_string(),
                Some(Err(_)) => "unavailable".to_string(),
            };
            let installed = shared.installed_hash.as_deref().map(short).unwrap_or("none");
            ui.label(
                RichText::new(format!("Latest {latest}  ·  Installed {installed}"))
                    .size(11.0)
                    .color(MUTED),
            );

            if let Some(Err(e)) = &shared.release {
                ui.label(RichText::new(e).size(11.0).color(AMBER));
            }
            if paths::get().dev_install {
                ui.label(RichText::new("Using your local build").size(11.0).color(AMBER));
            }
        });
    }

    fn installs(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        ui.label(RichText::new("DISCORD").size(10.0).color(MUTED));
        if self.installs.is_empty() {
            ui.label(
                RichText::new("No Discord install found. Install Discord first (snap isn't supported).").color(MUTED),
            );
        }
        for i in 0..self.installs.len() {
            let di = &self.installs[i];
            let selected = self.selected == i;
            let stroke = if selected {
                egui::Stroke::new(1.0_f32, Color32::from_white_alpha(110))
            } else {
                egui::Stroke::new(1.0_f32, LINE)
            };
            let response = egui::Frame::default()
                .fill(if selected { GLASS_HOVER } else { GLASS })
                .stroke(stroke)
                .corner_radius(14.0)
                .inner_margin(egui::Margin::symmetric(14, 10))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let mut branch = di.branch.clone();
                        if let Some(first) = branch.get_mut(0..1) {
                            first.make_ascii_uppercase();
                        }
                        ui.label(RichText::new(branch).size(14.0).strong());
                        if di.patched {
                            egui::Frame::default()
                                .fill(Color32::from_white_alpha(22))
                                .corner_radius(8.0)
                                .inner_margin(egui::Margin::symmetric(7, 2))
                                .show(ui, |ui| ui.label(RichText::new("Installed").size(10.0)));
                        }
                    });
                    ui.label(RichText::new(di.path.display().to_string()).size(11.0).color(MUTED));
                })
                .response
                .interact(egui::Sense::click());
            if response.clicked() {
                self.selected = i;
            }
            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }
    }

    fn buttons(&mut self, ui: &mut egui::Ui) {
        let busy = self.busy.load(Ordering::SeqCst);
        let has_install = !self.installs.is_empty();
        let patched = self.installs.get(self.selected).is_some_and(|d| d.patched);
        let openasar = self.openasar.get(self.selected).copied().unwrap_or(false);

        ui.add_space(6.0);
        ui.add_enabled_ui(!busy && has_install, |ui| {
            let primary = egui::Button::new(
                RichText::new(if patched {
                    "Reinstall EnderCord"
                } else {
                    "Install EnderCord"
                })
                .size(15.0)
                .strong()
                .color(BG),
            )
            .fill(Color32::WHITE)
            .stroke(egui::Stroke::NONE)
            .corner_radius(22.0);
            if ui.add_sized([ui.available_width(), 44.0], primary).clicked() {
                self.request(ui.ctx(), Action::Install);
            }

            ui.columns(4, |cols| {
                let full = |ui: &egui::Ui| [ui.available_width(), 34.0];
                let size = full(&cols[0]);
                if cols[0]
                    .add_sized(size, glass_button("Repair"))
                    .on_hover_text("Download the latest build and reinstall")
                    .clicked()
                {
                    self.request(cols[0].ctx(), Action::Repair);
                }
                let uninstall = egui::Button::new(RichText::new("Uninstall").size(13.0).color(RED));
                let size = full(&cols[1]);
                if cols[1]
                    .add_enabled_ui(patched, |ui| ui.add_sized(size, uninstall))
                    .inner
                    .clicked()
                {
                    self.request(cols[1].ctx(), Action::Uninstall);
                }
                let (label, action) = if openasar {
                    ("No OpenAsar", Action::UninstallOpenAsar)
                } else {
                    ("OpenAsar", Action::InstallOpenAsar)
                };
                let size = full(&cols[2]);
                if cols[2]
                    .add_sized(size, glass_button(label))
                    .on_hover_text("OpenAsar makes Discord start faster")
                    .clicked()
                {
                    self.request(cols[2].ctx(), action);
                }
                let size = full(&cols[3]);
                if cols[3].add_sized(size, glass_button("Refresh")).clicked() {
                    self.scan();
                }
            });
        });

        if let Some(action) = self.confirm {
            egui::Modal::new(egui::Id::new("discord-open"))
                .backdrop_color(Color32::from_black_alpha(170))
                .frame(
                    egui::Frame::default()
                        .fill(Color32::from_rgb(0x16, 0x16, 0x18))
                        .stroke(egui::Stroke::new(1.0_f32, LINE))
                        .corner_radius(18.0)
                        .inner_margin(22),
                )
                .show(ui.ctx(), |ui| {
                    ui.set_width(320.0);
                    ui.vertical_centered(|ui| {
                        ui.label(RichText::new("Discord is still open").size(17.0).strong());
                        ui.add_space(4.0);
                        ui.label(
                            RichText::new("Quit it from the tray icon (right click, Quit Discord) so it keeps you logged in. Force closing it logs you out.")
                                .size(12.0)
                                .color(MUTED),
                        );
                        ui.add_space(12.0);
                        let primary = egui::Button::new(RichText::new("I Quit It, Continue").strong().color(BG))
                            .fill(Color32::WHITE)
                            .corner_radius(18.0);
                        if ui.add_sized([ui.available_width(), 36.0], primary).clicked() {
                            self.confirm = None;
                            self.request(ui.ctx(), action);
                        }
                        ui.columns(2, |cols| {
                            let size = [cols[0].available_width(), 32.0];
                            let force = egui::Button::new(RichText::new("Force Close").size(13.0).color(RED));
                            if cols[0].add_sized(size, force).clicked() {
                                self.confirm = None;
                                self.run(cols[0].ctx(), action);
                            }
                            if cols[1].add_sized(size, glass_button("Cancel")).clicked() {
                                self.confirm = None;
                            }
                        });
                    });
                });
        }

        let last = HISTORY.lock().unwrap().last().cloned();
        ui.horizontal(|ui| {
            if busy {
                ui.spinner();
                ui.label(RichText::new("Working").color(MUTED));
            } else if let Some((level, msg)) = last {
                let color = match level {
                    Level::Error => RED,
                    Level::Warn => AMBER,
                    _ => MUTED,
                };
                ui.label(RichText::new(msg).size(12.0).color(color));
                if self.finished.load(Ordering::SeqCst) {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(glass_button("Open Discord")).clicked() {
                            if let Some(Err(e)) = self.installs.get(self.selected).map(discord::launch) {
                                crate::warn!("{e}");
                            }
                        }
                    });
                }
            }
        });
    }

    fn log(&self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new(RichText::new("Details").size(11.0).color(MUTED))
            .default_open(false)
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(110.0)
                    .stick_to_bottom(true)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        for (level, msg) in HISTORY.lock().unwrap().iter() {
                            let color = match level {
                                Level::Error => RED,
                                Level::Warn => AMBER,
                                _ => MUTED,
                            };
                            ui.label(RichText::new(msg).size(11.0).monospace().color(color));
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
                .frame(
                    egui::Frame::default()
                        .fill(Color32::from_white_alpha(14))
                        .inner_margin(8),
                )
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        ui.label(RichText::new(format!("Installer {tag} is available")).size(12.0));
                        if ui
                            .add_enabled(!self.busy.load(Ordering::SeqCst), glass_button("Update"))
                            .clicked()
                        {
                            self.run(ctx, Action::UpdateSelf);
                        }
                    });
                });
        }

        egui::TopBottomPanel::bottom("footer")
            .frame(egui::Frame::default().fill(BG).inner_margin(10))
            .show_separator_line(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} · {GIT_HASH}", TAG.unwrap_or("dev")))
                            .size(10.0)
                            .color(MUTED),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.hyperlink_to(RichText::new("Support").size(11.0), crate::constants::SUPPORT_URL);
                        ui.hyperlink_to(
                            RichText::new("Source").size(11.0),
                            format!("https://github.com/{}", crate::constants::REPO),
                        );
                    });
                });
            });

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(BG)
                    .inner_margin(egui::Margin::symmetric(36, 8)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.header(ui);
                    self.installs(ui);
                    self.buttons(ui);
                    self.log(ui);
                });
            });
    }
}

pub fn run() {
    let (pixels, w, h) = load_icon();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("EnderCord Installer")
            .with_inner_size([520.0, 660.0])
            .with_min_inner_size([460.0, 560.0])
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
