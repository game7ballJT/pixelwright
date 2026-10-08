//! Window › Swatches: the swatch library in named groups.
//!
//! Click a swatch to make it the foreground colour; right-click for "Set as Background" and
//! "Delete Swatch". The toolbar adds the foreground colour, builds a palette from the open image,
//! imports `.aco` / `.ase` files (File › Open and drag-and-drop route them here too, see
//! `preset_files_ui`) and exports a library. Every change goes through the `swatch.presets.*`
//! engine commands, so agents and the CLI see the same library.

use egui::{Color32, Rect, RichText, Sense, Stroke, StrokeKind, Vec2, vec2};
use serde_json::{Value, json};

use crate::theme::Tokens;
use crate::{PixelwrightApp, icons};

/// The group the toolbar's Add button fills, so the built-in Default group stays as shipped.
const USER_GROUP: &str = "My Swatches";
const COLUMNS: usize = 10;

enum Action {
    Add,
    FromImage,
    Import,
    Export { ase: bool },
    Foreground([f32; 3]),
    Background([f32; 3]),
    Delete(String),
}

/// What the panel draws: group name → (swatch name, colour).
type Library = Vec<(String, Vec<(String, [f32; 3])>)>;

fn library(app: &PixelwrightApp) -> Library {
    app.session.presets.swatches.iter().map(|g| (g.name.clone(), g.items.iter().map(|s| (s.name.clone(), s.color)).collect())).collect()
}

/// Where the first swatch was drawn this frame (tests click it).
#[cfg(test)]
pub fn first_rect_id() -> egui::Id {
    egui::Id::new("swatches-first-rect")
}

pub fn panel(app: &mut PixelwrightApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let lib = library(app);
    let mut action: Option<Action> = None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        if icons::button(ui, "plus", 26.0, false, tl!("Add the foreground color as a swatch")).clicked() {
            action = Some(Action::Add);
        }
        if icons::button(ui, "image", 26.0, false, tl!("Make a palette from the image")).clicked() {
            action = Some(Action::FromImage);
        }
        if icons::button(ui, "folder-open", 26.0, false, tl!("Import swatches…")).clicked() {
            action = Some(Action::Import);
        }
        ui.menu_button(tl!("Export"), |ui| {
            if ui.button(tl!("Swatch Exchange (.ase)")).clicked() {
                action = Some(Action::Export { ase: true });
                ui.close();
            }
            if ui.button(tl!("Color Swatches (.aco)")).clicked() {
                action = Some(Action::Export { ase: false });
                ui.close();
            }
        });
    });
    ui.add_space(4.0);

    let gap = 4.0;
    let w = ui.available_width();
    let cell = ((w - gap * (COLUMNS as f32 - 1.0)) / COLUMNS as f32).floor().max(8.0);
    for (gi, (group, items)) in lib.iter().enumerate() {
        if lib.len() > 1 {
            ui.label(RichText::new(group).small().color(t.text_dim));
        }
        let rows = items.len().div_ceil(COLUMNS);
        let (area, _) = ui.allocate_exact_size(vec2(w, rows as f32 * (cell + gap)), Sense::hover());
        for (i, (name, c)) in items.iter().enumerate() {
            let (cx, cy) = ((i % COLUMNS) as f32, (i / COLUMNS) as f32);
            let r = Rect::from_min_size(area.min + vec2(cx * (cell + gap), cy * (cell + gap)), Vec2::splat(cell));
            let rgb = [(c[0] * 255.0).round() as u8, (c[1] * 255.0).round() as u8, (c[2] * 255.0).round() as u8];
            #[cfg(test)]
            if gi == 0 && i == 0 {
                ui.ctx().data_mut(|d| d.insert_temp(first_rect_id(), r));
            }
            let resp = ui.interact(r, ui.id().with(("sw", gi, i)), Sense::click());
            ui.painter().rect_filled(r, 4.0, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
            if resp.hovered() {
                ui.painter().rect_stroke(r, 4.0, Stroke::new(1.5, t.text), StrokeKind::Outside);
            }
            let resp = resp.on_hover_text(format!("{name}  #{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2]));
            if resp.clicked() {
                action = Some(Action::Foreground(*c));
            }
            egui::Popup::context_menu(&resp).show(|ui| {
                if ui.button(tl!("Set as Background")).clicked() {
                    action = Some(Action::Background(*c));
                    ui.close();
                }
                if ui.button(tl!("Delete Swatch")).clicked() {
                    action = Some(Action::Delete(name.clone()));
                    ui.close();
                }
            });
        }
        ui.add_space(4.0);
    }
    ui.label(RichText::new(tl!("Click sets foreground · right-click for more")).small().color(t.text_faint));

    if let Some(a) = action {
        run(app, a);
    }
}

fn say(app: &mut PixelwrightApp, msg: String, error: bool) {
    app.ui.status = msg;
    app.ui.status_error = error;
}

fn run(app: &mut PixelwrightApp, action: Action) {
    match action {
        Action::Foreground(c) => {
            app.session.tools.foreground = [c[0], c[1], c[2], 1.0];
            crate::type_tool::foreground_changed(app);
        }
        Action::Background(c) => app.session.tools.background = [c[0], c[1], c[2], 1.0],
        Action::Add => match app.run("swatch.presets.add", json!({"group": USER_GROUP})) {
            Ok(r) => say(app, format!("Added swatch {} to {}", r["color"].as_str().unwrap_or_default(), USER_GROUP), false),
            Err(e) => say(app, e, true),
        },
        Action::Delete(name) => {
            if let Err(e) = app.run("swatch.presets.edit", json!({"action": "delete", "preset": name})) {
                say(app, e, true);
            }
        }
        Action::FromImage => match app.run("swatch.presets.fromImage", json!({"count": 8})) {
            Ok(r) => say(app, format!("Made the palette \"{}\" from the image", r["group"].as_str().unwrap_or_default()), false),
            Err(e) => say(app, e, true),
        },
        // File › Open routes .aco and .ase files here (`preset_files_ui`).
        Action::Import => app.open_dialog_file(),
        Action::Export { ase } => {
            let id = if ase { "swatch.presets.exportAse" } else { "swatch.presets.exportAco" };
            let result = export(app, id);
            match result {
                Ok(msg) => say(app, msg, false),
                Err(e) if e == "cancelled" => {}
                Err(e) => say(app, e, true),
            }
        }
    }
}

/// Export every group: ask for a path (the browser build downloads under the suggested name),
/// then write the file the command returns.
fn export(app: &mut PixelwrightApp, id: &str) -> Result<String, String> {
    let r: Value = app.run(id, json!({}))?;
    let name = r["fileName"].as_str().unwrap_or("Swatches.ase").to_string();
    let data = r["data"].as_str().and_then(pixelwright_engine::paint::tile::b64_decode).ok_or("the export produced no data")?;
    let path = app.services.pick_save.as_mut().and_then(|p| p(&name)).ok_or("cancelled")?;
    let write = app.services.write.as_mut().ok_or("no writer configured")?;
    write(&path, &data)?;
    Ok(format!("Exported swatches to {path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> PixelwrightApp {
        PixelwrightApp::new(pixelwright_engine::Session::new(), Default::default())
    }

    #[test]
    fn the_panel_shows_the_library_and_actions_change_it() {
        let mut a = app();
        a.session.tools.foreground = [0.1, 0.2, 0.3, 1.0];
        assert_eq!(library(&a).len(), 1);
        assert_eq!(library(&a)[0].1.len(), 40);
        run(&mut a, Action::Add);
        let lib = library(&a);
        assert_eq!(lib.len(), 2, "a My Swatches group appears");
        assert_eq!(lib[1].0, USER_GROUP);
        assert_eq!(lib[1].1[0].1, [0.1, 0.2, 0.3]);
        run(&mut a, Action::Background([1.0, 0.0, 0.0]));
        assert_eq!(a.session.tools.background[..3], [1.0, 0.0, 0.0]);
        run(&mut a, Action::Foreground([0.0, 1.0, 0.0]));
        assert_eq!(a.session.tools.foreground[..3], [0.0, 1.0, 0.0]);
        let name = lib[1].1[0].0.clone();
        run(&mut a, Action::Delete(name));
        assert!(library(&a)[1].1.is_empty(), "the swatch is gone");
        assert!(!a.ui.status_error, "{}", a.ui.status);
    }

    #[test]
    fn a_palette_needs_an_image_and_says_so() {
        let mut a = app();
        run(&mut a, Action::FromImage);
        assert!(a.ui.status_error, "no document: {}", a.ui.status);
        a.run("file.new", json!({"width": 16, "height": 16, "background": "#336699"})).unwrap();
        run(&mut a, Action::FromImage);
        assert!(!a.ui.status_error, "{}", a.ui.status);
        assert!(library(&a).iter().any(|(g, items)| g.starts_with("Palette") && !items.is_empty()));
    }

    #[test]
    fn export_writes_through_the_services() {
        use std::sync::{Arc, Mutex};
        let written: Arc<Mutex<Vec<(String, usize)>>> = Arc::default();
        let w = written.clone();
        let mut a = app();
        a.services.pick_save = Some(Box::new(|name: &str| Some(format!("/tmp/{name}"))));
        a.services.write = Some(Box::new(move |path: &str, bytes: &[u8]| {
            w.lock().unwrap().push((path.to_string(), bytes.len()));
            Ok(())
        }));
        run(&mut a, Action::Export { ase: true });
        run(&mut a, Action::Export { ase: false });
        let got = written.lock().unwrap().clone();
        assert_eq!(got.len(), 2, "{}", a.ui.status);
        assert!(got[0].0.ends_with("Default.ase") || got[0].0.ends_with("Swatches.ase"), "{}", got[0].0);
        assert!(got[0].1 > 100 && got[1].1 > 100);
        // Cancelling the dialog is quiet.
        a.services.pick_save = Some(Box::new(|_: &str| None));
        a.ui.status_error = false;
        run(&mut a, Action::Export { ase: true });
        assert!(!a.ui.status_error);
    }
}
