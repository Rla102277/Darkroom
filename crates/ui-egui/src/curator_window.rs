//! IA Darkroom: the Curator window (Window > Curator…), `CURATOR.md` C0 and C1.
//!
//! One floating window: the scan and its live count at the top, then three tabs. Report is the
//! library in numbers. Stacks and Cull are review sheets: the groups of the newest plan on the
//! left (accept each with its checkbox), the frames of the chosen group on the right as a contact
//! sheet (click a frame to take it out of the plan, click again to put it back), and at the
//! bottom what applying would write, behind a confirm step. Nothing reaches Photos before that,
//! and nothing is ever deleted or hidden: a plan only adds keywords.
//!
//! Everything goes through the `curator.*` commands, so the window, the CLI and MCP do the same
//! thing. Thumbnails come from the helper (Photos' own previews, 256 px) on a worker.

use std::collections::HashMap;
use std::sync::mpsc;

use egui::{Color32, RichText, Ui, vec2};
use serde_json::{Value, json};

use crate::LightcraftApp;
use crate::theme::Tokens;

const PAGE: usize = 60;
const THUMB: u32 = 256;
const CELL: f32 = 132.0;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Report,
    Stacks,
    Cull,
}

impl Tab {
    fn job(self) -> &'static str {
        match self {
            Tab::Stacks => "stacks",
            _ => "cull",
        }
    }
}

enum Thumb {
    Pending,
    Ready(egui::TextureHandle),
    Failed,
}

#[derive(Default)]
pub struct CuratorWindow {
    pub open: bool,
    tab: Tab,
    status: Value,
    report: Option<String>,
    plan: Value,
    group: Option<String>,
    offset: usize,
    items: Vec<Value>,
    thumbs: HashMap<String, Thumb>,
    rx: Option<mpsc::Receiver<(String, Option<egui::ColorImage>)>>,
    confirming: bool,
    message: Option<(String, bool)>,
    /// The job that was running last frame: its end refreshes the counts.
    was_running: bool,
}

impl CuratorWindow {
    fn refresh_status(&mut self, app: &mut LightcraftApp) {
        match app.run("curator.status", Value::Null) {
            Ok(v) => self.status = v,
            Err(e) => self.message = Some((e, true)),
        }
    }

    fn load_plan(&mut self, app: &mut LightcraftApp) {
        self.items.clear();
        self.confirming = false;
        match app.run("curator.plan", json!({"job": self.tab.job()})) {
            Ok(v) => {
                let first = v["groups"].as_array().and_then(|g| g.first()).and_then(|g| g["key"].as_str()).map(str::to_string);
                self.plan = v;
                if self.group.as_ref().is_none_or(|g| !self.plan["groups"].as_array().into_iter().flatten().any(|x| x["key"] == g.as_str())) {
                    self.group = first;
                    self.offset = 0;
                }
                self.load_items(app);
            }
            Err(_) => {
                self.plan = Value::Null;
                self.group = None;
            }
        }
    }

    fn load_items(&mut self, app: &mut LightcraftApp) {
        let Some(group) = self.group.clone() else { return };
        let id = self.plan["id"].as_str().unwrap_or_default().to_string();
        match app.run("curator.plan", json!({"id": id, "group": group, "offset": self.offset, "limit": PAGE})) {
            Ok(v) => {
                self.items = v["items"].as_array().cloned().unwrap_or_default();
                self.plan = v;
                self.fetch_thumbs(app);
            }
            Err(e) => self.message = Some((e, true)),
        }
    }

    /// Thumbnails for the shown items that have none yet, on a worker.
    fn fetch_thumbs(&mut self, app: &LightcraftApp) {
        let Some(photos) = app.session.photos.clone() else { return };
        let want: Vec<String> =
            self.items.iter().filter_map(|i| i["localId"].as_str()).filter(|id| !self.thumbs.contains_key(*id)).map(str::to_string).collect();
        if want.is_empty() {
            return;
        }
        for id in &want {
            self.thumbs.insert(id.clone(), Thumb::Pending);
        }
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        let _ = std::thread::Builder::new().name("curator-thumbs".into()).spawn(move || {
            for chunk in want.chunks(20) {
                let files = photos.thumbs(chunk, THUMB, None).unwrap_or_default();
                let mut done: std::collections::HashSet<String> = Default::default();
                for f in files {
                    let img = f
                        .path
                        .as_deref()
                        .and_then(|p| lightcraft_engine::cmd::curator::thumbnail_rgba(std::path::Path::new(p), THUMB))
                        .map(|(w, h, px)| egui::ColorImage::from_rgba_unmultiplied([w, h], &px));
                    done.insert(f.id.clone());
                    if tx.send((f.id, img)).is_err() {
                        return;
                    }
                }
                for id in chunk.iter().filter(|id| !done.contains(*id)) {
                    if tx.send((id.clone(), None)).is_err() {
                        return;
                    }
                }
            }
        });
    }

    fn pump(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else { return };
        let mut got = false;
        while let Ok((id, img)) = rx.try_recv() {
            got = true;
            let t = match img {
                Some(img) => Thumb::Ready(ctx.load_texture(format!("curator-{id}"), img, egui::TextureOptions::LINEAR)),
                None => Thumb::Failed,
            };
            self.thumbs.insert(id, t);
        }
        if got || self.thumbs.values().any(|t| matches!(t, Thumb::Pending)) {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }
}

/// Opens the window (Window > Curator…).
pub fn open(app: &mut LightcraftApp) {
    let mut w = std::mem::take(&mut app.curator);
    w.open = true;
    w.refresh_status(app);
    app.curator = w;
}

pub fn show(app: &mut LightcraftApp, ctx: &egui::Context) {
    if !app.curator.open {
        return;
    }
    let mut w = std::mem::take(&mut app.curator);
    w.pump(ctx);
    let job = app.session.curator.info();
    if w.was_running && !job.running {
        w.refresh_status(app);
        let msg = match (&job.error, job.kind.as_str()) {
            (Some(e), _) => (format!("The {} stopped: {e}", job.kind), true),
            (None, "scan") => (
                format!(
                    "Scan done: {} photos, {} measured this run{}.",
                    job.result["photos"],
                    job.result["measured"],
                    if job.result["cancelled"] == true { ", stopped early" } else { "" }
                ),
                false,
            ),
            (None, _) => (format!("Wrote keywords to {} photos in Apple Photos.", job.result["written"]), false),
        };
        w.message = Some(msg);
        w.report = None;
        if job.kind == "apply" {
            w.load_plan(app);
        }
    }
    w.was_running = job.running;
    if job.running {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
    let mut open = true;
    egui::Window::new(crate::i18n::tr("Curator"))
        .id(egui::Id::new("curator-window"))
        .open(&mut open)
        .resizable(true)
        .collapsible(false)
        .default_size(vec2(1020.0, 700.0))
        .show(ctx, |ui| body(app, &mut w, ui, &job));
    w.open = open && w.open;
    app.curator = w;
}

fn body(app: &mut LightcraftApp, w: &mut CuratorWindow, ui: &mut Ui, job: &lightcraft_engine::cmd::curator::JobInfo) {
    let t = Tokens::get(ui.ctx());
    // the scan
    ui.horizontal(|ui| {
        let photos_ready = app.session.photos.is_some();
        if ui
            .add_enabled(photos_ready && !job.running, egui::Button::new("Scan Apple Photos"))
            .on_hover_text("Reads every photo's facts and measures the new and edited ones. Nothing in Photos changes.")
            .clicked()
        {
            match app.run("curator.scan", json!({"wait": false})) {
                Ok(_) => w.message = None,
                Err(e) => w.message = Some((e, true)),
            }
        }
        if job.running && ui.button("Stop").clicked() {
            let _ = app.run("curator.cancel", Value::Null);
        }
        ui.add_space(8.0);
        if job.running {
            ui.spinner();
            ui.label(RichText::new(format!("{}: {} of {}", job.phase, job.done, job.total)).color(t.text));
        } else {
            let st = &w.status["store"];
            let scanned = st["scanned"].as_str().map(|s| s.replace('T', " ").trim_end_matches('Z').to_string());
            ui.label(
                RichText::new(match scanned {
                    Some(when) => format!("{} photos, {} measured, {} albums. Scanned {when} UTC.", st["photos"], st["measured"], st["albums"]),
                    None => "Not scanned yet.".into(),
                })
                .color(t.text_dim),
            );
        }
        if !photos_ready {
            ui.label(RichText::new("The Apple Photos helper is not available.").color(t.caution));
        }
    });
    if let Some((m, error)) = &w.message {
        ui.label(RichText::new(m).color(if *error { t.caution } else { t.text_label }));
    }
    ui.separator();
    ui.horizontal(|ui| {
        for (tab, label) in [(Tab::Report, "Report"), (Tab::Stacks, "Stacks"), (Tab::Cull, "Cull")] {
            let r = ui.selectable_label(w.tab == tab, label);
            crate::widgets::register(ui.ctx(), &format!("curator:tab.{label}"), r.rect);
            if r.clicked() && w.tab != tab {
                w.tab = tab;
                w.group = None;
                w.offset = 0;
                if tab != Tab::Report {
                    w.load_plan(app);
                }
            }
        }
    });
    ui.separator();
    match w.tab {
        Tab::Report => report(app, w, ui, &t),
        Tab::Stacks | Tab::Cull => review(app, w, ui, &t, job.running),
    }
}

fn report(app: &mut LightcraftApp, w: &mut CuratorWindow, ui: &mut Ui, t: &Tokens) {
    if w.report.is_none() {
        w.report = Some(match app.run("curator.report", Value::Null) {
            Ok(v) => v["text"].as_str().unwrap_or_default().to_string(),
            Err(e) => e,
        });
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.label(RichText::new(w.report.as_deref().unwrap_or_default()).monospace().color(t.text));
    });
}

fn review(app: &mut LightcraftApp, w: &mut CuratorWindow, ui: &mut Ui, t: &Tokens, busy: bool) {
    let job = w.tab.job();
    ui.horizontal(|ui| {
        let label = if job == "stacks" { "Find Stacks" } else { "Run Cull" };
        if ui.add_enabled(!busy, egui::Button::new(label)).on_hover_text("Makes a new plan from the last scan. Nothing in Photos changes.").clicked()
        {
            match app.run(&format!("curator.{job}"), Value::Null) {
                Ok(_) => {
                    w.group = None;
                    w.load_plan(app);
                    w.message = None;
                }
                Err(e) => w.message = Some((e, true)),
            }
        }
        if w.plan.is_object() {
            let applied = w.plan["applied"].as_str();
            ui.label(
                RichText::new(format!(
                    "Plan {}{}",
                    w.plan["id"].as_str().unwrap_or(""),
                    applied.map(|a| format!(", applied {a}")).unwrap_or_default()
                ))
                .color(t.text_dim),
            );
        }
    });
    if !w.plan.is_object() {
        ui.label(RichText::new("No plan yet. Scan, then press the button above.").color(t.text_dim));
        return;
    }
    let applied = w.plan["applied"].is_string();
    let groups = w.plan["groups"].as_array().cloned().unwrap_or_default();
    let plan_id = w.plan["id"].as_str().unwrap_or_default().to_string();
    let avail = ui.available_size();
    ui.horizontal_top(|ui| {
        // groups
        ui.vertical(|ui| {
            ui.set_width(250.0);
            for g in &groups {
                let key = g["key"].as_str().unwrap_or_default().to_string();
                ui.horizontal(|ui| {
                    let mut accepted = g["accepted"].as_bool().unwrap_or(false);
                    if ui.add_enabled(!applied, egui::Checkbox::without_text(&mut accepted)).on_hover_text("Accept this group").changed() {
                        if let Err(e) = app.run("curator.review", json!({"id": plan_id, "group": key, "accept": accepted})) {
                            w.message = Some((e, true));
                        }
                        w.load_plan(app);
                    }
                    let out = g["excluded"].as_u64().unwrap_or(0);
                    let text = if out > 0 {
                        format!("{}  {} ({} out)", g["label"].as_str().unwrap_or(""), g["count"], out)
                    } else {
                        format!("{}  {}", g["label"].as_str().unwrap_or(""), g["count"])
                    };
                    if ui.selectable_label(w.group.as_deref() == Some(key.as_str()), text).clicked() {
                        w.group = Some(key.clone());
                        w.offset = 0;
                        w.load_items(app);
                    }
                });
            }
            ui.add_space(12.0);
            let tw = &w.plan["toApply"];
            let photos = tw["photos"].as_u64().unwrap_or(0);
            let kw: Vec<String> = tw["keywords"].as_object().map(|o| o.iter().map(|(k, n)| format!("{k}: {n}")).collect()).unwrap_or_default();
            ui.label(RichText::new(format!("Applying writes to {photos} photos")).color(t.text));
            for k in &kw {
                ui.label(RichText::new(format!("  {k}")).color(t.text_dim));
            }
            ui.label(RichText::new("Keywords only. Nothing is deleted or hidden.").small().color(t.text_dim));
            ui.add_space(6.0);
            if !w.confirming {
                if ui.add_enabled(!applied && !busy && photos > 0, egui::Button::new("Apply to Apple Photos…")).clicked() {
                    w.confirming = true;
                }
            } else {
                ui.label(RichText::new(format!("Add these keywords to {photos} photos in Apple Photos?")).color(t.caution));
                ui.horizontal(|ui| {
                    if ui.button("Write").clicked() {
                        w.confirming = false;
                        match app.run("curator.apply", json!({"id": plan_id, "confirm": true, "wait": false})) {
                            Ok(_) => w.message = Some(("Writing to Apple Photos…".into(), false)),
                            Err(e) => w.message = Some((e, true)),
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        w.confirming = false;
                    }
                });
            }
        });
        ui.separator();
        // the contact sheet
        ui.vertical(|ui| {
            let Some(key) = w.group.clone() else { return };
            let count = groups.iter().find(|g| g["key"] == key.as_str()).and_then(|g| g["count"].as_u64()).unwrap_or(0) as usize;
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!(
                        "{} to {} of {count}. Click a frame to take it out of the plan.",
                        w.offset + 1,
                        (w.offset + PAGE).min(count)
                    ))
                    .color(t.text_dim),
                );
                if ui.add_enabled(w.offset > 0, egui::Button::new("Previous")).clicked() {
                    w.offset = w.offset.saturating_sub(PAGE);
                    w.load_items(app);
                }
                if ui.add_enabled(w.offset + PAGE < count, egui::Button::new("Next")).clicked() {
                    w.offset += PAGE;
                    w.load_items(app);
                }
            });
            let mut toggle: Option<(String, bool)> = None;
            egui::ScrollArea::vertical().max_height(avail.y - 40.0).auto_shrink([false, false]).show(ui, |ui| {
                let cols = ((ui.available_width() / (CELL + 8.0)).floor() as usize).max(1);
                egui::Grid::new("curator-sheet").spacing(vec2(8.0, 8.0)).show(ui, |ui| {
                    for (n, it) in w.items.iter().enumerate() {
                        let id = it["id"].as_str().unwrap_or_default();
                        let local = it["localId"].as_str().unwrap_or_default();
                        let excluded = it["excluded"].as_bool().unwrap_or(false);
                        let (rect, resp) = ui.allocate_exact_size(vec2(CELL, CELL + 16.0), egui::Sense::click());
                        let img_rect = egui::Rect::from_min_size(rect.min, vec2(CELL, CELL));
                        ui.painter().rect_filled(img_rect, 2.0, t.cell);
                        match w.thumbs.get(local) {
                            Some(Thumb::Ready(tex)) => {
                                let s = tex.size_vec2();
                                let k = (CELL / s.x.max(1.0)).min(CELL / s.y.max(1.0));
                                let r = egui::Rect::from_center_size(img_rect.center(), s * k);
                                let tint = if excluded { Color32::from_white_alpha(70) } else { Color32::WHITE };
                                ui.painter().image(tex.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), tint);
                            }
                            Some(Thumb::Failed) => {
                                ui.painter().text(
                                    img_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "no preview",
                                    egui::FontId::proportional(11.0),
                                    t.text_dim,
                                );
                            }
                            _ => {
                                ui.painter().text(img_rect.center(), egui::Align2::CENTER_CENTER, "…", egui::FontId::proportional(14.0), t.text_dim);
                            }
                        }
                        if excluded {
                            ui.painter().text(
                                img_rect.right_top() + vec2(-8.0, 8.0),
                                egui::Align2::RIGHT_TOP,
                                "OUT",
                                egui::FontId::proportional(12.0),
                                t.caution,
                            );
                        }
                        let name = it["filename"].as_str().unwrap_or(id);
                        ui.painter().text(
                            egui::pos2(rect.min.x + 2.0, rect.max.y - 2.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("{} {:.0}%", truncate(name, 16), it["confidence"].as_f64().unwrap_or(0.0) * 100.0),
                            egui::FontId::proportional(10.5),
                            if excluded { t.text_dim } else { t.text_label },
                        );
                        let resp =
                            resp.on_hover_text(format!("{name}\n{}\n{}", it["reason"].as_str().unwrap_or(""), it["created"].as_str().unwrap_or("")));
                        if resp.clicked() && !applied {
                            toggle = Some((id.to_string(), !excluded));
                        }
                        if (n + 1) % cols == 0 {
                            ui.end_row();
                        }
                    }
                });
            });
            if let Some((id, out)) = toggle {
                let k = if out { "exclude" } else { "include" };
                if let Err(e) = app.run("curator.review", json!({"id": plan_id, "group": key, k: [id]})) {
                    w.message = Some((e, true));
                }
                w.load_items(app);
            }
        });
    });
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n { s.to_string() } else { format!("{}…", s.chars().take(n.saturating_sub(1)).collect::<String>()) }
}
