//! Window › Swatches: a swatch library in named groups.
//!
//! The panel used to show a fixed set of 40 colours. It now shows these groups, which users can
//! extend (Add Swatch, Palette from Image), import from Adobe Color Swatch (`.aco`) and Swatch
//! Exchange (`.ase`) files, export, rename, move and delete, all through `swatch.presets.*`
//! commands so the UI, the CLI and MCP share them. The groups persist with the preferences
//! document like the gradient presets.
//!
//! Colours are stored as sRGB. Imported HSB, Lab and Gray colours are converted exactly; CMYK is
//! converted with the plain `(1 − c)(1 − k)` formula, since a swatch file carries no profile.

use pixelwright_algo::quantize::{self, Forced, PaletteKind};
use pixelwright_color::PixelFormat;
use pixelwright_psd::swatches::{self as files, SwatchColor, SwatchGroup};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::{Group, Named, always, bad, edit_groups, find, group_index, req_str, str_param, unique_name};
use crate::commands::CommandSpec;
use crate::{EngineError, Result, Session};

/// One swatch: a name and an sRGB colour (0..1).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Swatch {
    pub name: String,
    pub color: [f32; 3],
}

impl Named for Swatch {
    fn name(&self) -> &str {
        &self.name
    }
    fn set_name(&mut self, n: String) {
        self.name = n;
    }
}

impl Swatch {
    pub fn new(name: &str, rgb: [u8; 3]) -> Self {
        Swatch { name: name.into(), color: rgb.map(|v| f32::from(v) / 255.0) }
    }

    /// `#RRGGBB`.
    pub fn hex(&self) -> String {
        hex_of(self.color)
    }

    pub fn to_json(&self) -> Value {
        json!({"name": self.name, "color": self.hex()})
    }
}

fn q(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// `#RRGGBB` (upper case) of an sRGB colour.
pub fn hex_of(c: [f32; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", q(c[0]), q(c[1]), q(c[2]))
}

/// `"#rgb"`, `"#rrggbb"` or `[r, g, b]` (0..1) as sRGB.
fn parse_color(v: &Value) -> Option<[f32; 3]> {
    match v {
        Value::String(s) => {
            let s = s.trim().trim_start_matches('#');
            let full = match s.len() {
                3 => s.chars().flat_map(|c| [c, c]).collect::<String>(),
                6 => s.to_string(),
                _ => return None,
            };
            let b = |i: usize| u8::from_str_radix(full.get(i..i + 2)?, 16).ok().map(|x| f32::from(x) / 255.0);
            Some([b(0)?, b(2)?, b(4)?])
        }
        Value::Array(a) if a.len() >= 3 => {
            let n = |i: usize| a.get(i).and_then(Value::as_f64).filter(|x| x.is_finite()).map(|x| (x as f32).clamp(0.0, 1.0));
            Some([n(0)?, n(1)?, n(2)?])
        }
        _ => None,
    }
}

/// The 40 colours the panel always had, now with names, as the "Default" group.
pub fn builtin() -> Vec<Group<Swatch>> {
    const NAMES: [&str; 40] = [
        "Black", "Gray 90", "Gray 80", "Gray 70", "Gray 60", "Gray 50", "Gray 40", "Gray 30", "Gray 20", "White", "Pastel Red", "Pastel Orange", "Pastel Yellow",
        "Pastel Lime", "Pastel Green", "Pastel Mint", "Pastel Sky", "Pastel Blue", "Pastel Violet", "Pastel Pink", "Red", "Orange", "Yellow", "Lime", "Green", "Teal",
        "Cyan", "Blue", "Violet", "Magenta", "Dark Red", "Brown", "Olive", "Moss", "Forest", "Deep Teal", "Deep Blue", "Navy", "Indigo", "Plum",
    ];
    const RGB: [[u8; 3]; 40] = [
        [0, 0, 0],
        [26, 26, 26],
        [51, 51, 51],
        [77, 77, 77],
        [102, 102, 102],
        [128, 128, 128],
        [153, 153, 153],
        [179, 179, 179],
        [204, 204, 204],
        [255, 255, 255],
        [236, 128, 128],
        [244, 176, 132],
        [250, 224, 128],
        [214, 240, 128],
        [150, 232, 150],
        [128, 232, 200],
        [128, 220, 240],
        [128, 176, 244],
        [168, 144, 244],
        [232, 144, 232],
        [230, 40, 40],
        [245, 120, 30],
        [250, 210, 30],
        [160, 220, 40],
        [40, 200, 80],
        [30, 200, 170],
        [30, 170, 230],
        [40, 100, 230],
        [120, 70, 220],
        [210, 50, 180],
        [120, 20, 20],
        [130, 60, 10],
        [130, 110, 10],
        [80, 120, 20],
        [20, 100, 40],
        [10, 100, 90],
        [10, 80, 120],
        [20, 50, 120],
        [60, 30, 110],
        [110, 20, 90],
    ];
    vec![Group::new("Default", NAMES.iter().zip(RGB).map(|(n, c)| Swatch::new(n, c)).collect())]
}

// ------------------------------------------------------------------ file conversion

/// A stored swatch colour as sRGB.
pub fn to_srgb(c: &SwatchColor) -> [f32; 3] {
    let clamp = |v: [f32; 3]| v.map(|x| if x.is_finite() { x.clamp(0.0, 1.0) } else { 0.0 });
    match *c {
        SwatchColor::Rgb(v) => clamp(v),
        SwatchColor::Hsb(v) => clamp(files::hsb_to_rgb(v)),
        SwatchColor::Cmyk([c, m, y, k]) => clamp([(1.0 - c) * (1.0 - k), (1.0 - m) * (1.0 - k), (1.0 - y) * (1.0 - k)]),
        SwatchColor::Lab(v) => clamp(pixelwright_color::convert::lab_to_srgb(v)),
        SwatchColor::Gray(g) => clamp([g, g, g]),
    }
}

fn from_file(file: &files::SwatchFile) -> Vec<Group<Swatch>> {
    file.groups
        .iter()
        .map(|g| Group::new(&g.name, g.swatches.iter().map(|s| Swatch { name: s.name.clone(), color: to_srgb(&s.color) }).collect()))
        .collect()
}

fn to_file(groups: &[&Group<Swatch>]) -> Vec<SwatchGroup> {
    groups
        .iter()
        .map(|g| SwatchGroup {
            name: g.name.clone(),
            swatches: g.items.iter().map(|s| files::Swatch { name: s.name.clone(), color: SwatchColor::Rgb(s.color) }).collect(),
        })
        .collect()
}

// ------------------------------------------------------------------ palette from an image

/// The `count` most prominent colours of the active document's composite, most prominent first
/// (or by `sort`: `"hue"`, `"lightness"`), with each colour's share of the opaque pixels.
///
/// The composite is sampled on a grid of at most 160 × 160 points; the palette is the perceptual
/// (Lab median cut + k-means) one the Indexed Color mode uses, and each sample is counted for its
/// nearest palette entry.
pub fn prominent_colors(s: &Session, count: usize, sort: &str) -> Result<Vec<([f32; 3], f32)>> {
    let doc = &s.active().ok_or(EngineError::NoDocument)?.doc;
    let f = doc.pixel_format();
    let comp = pixelwright_compose::flatten_to_surface(doc, PixelFormat::new(f.mode, f.sample, true), None);
    let b = doc.bounds();
    let (w, h) = (b.width() as usize, b.height() as usize);
    if w == 0 || h == 0 {
        return Err(EngineError::Other("the document is empty".into()));
    }
    let step = w.max(h).div_ceil(160).max(1);
    let mut samples: Vec<[f32; 4]> = Vec::new();
    for y in (0..h).step_by(step) {
        for x in (0..w).step_by(step) {
            let p = comp.rgba(b.x0 + x as i32, b.y0 + y as i32);
            if p[3] >= 0.5 && p.iter().all(|v| v.is_finite()) {
                samples.push([p[0].clamp(0.0, 1.0), p[1].clamp(0.0, 1.0), p[2].clamp(0.0, 1.0), 1.0]);
            }
        }
    }
    if samples.is_empty() {
        return Err(EngineError::Other("the image has no opaque pixels to take colours from".into()));
    }
    let pal = quantize::build_palette(&samples, PaletteKind::Perceptual, count.clamp(2, 64), Forced::None).map_err(EngineError::Other)?;
    let mut counts = vec![0usize; pal.len()];
    for p in &samples {
        let best = pal
            .iter()
            .enumerate()
            .map(|(i, e)| (i, (0..3).map(|k| (p[k] * 255.0 - f32::from(e[k])).powi(2)).sum::<f32>()))
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(i, _)| i);
        if let Some(c) = best.and_then(|i| counts.get_mut(i)) {
            *c += 1;
        }
    }
    let total = samples.len() as f32;
    let mut out: Vec<([f32; 3], f32)> =
        pal.iter().zip(&counts).filter(|(_, n)| **n > 0).map(|(e, n)| (e.map(|v| f32::from(v) / 255.0), *n as f32 / total)).collect();
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out.truncate(count.clamp(2, 64));
    match sort {
        "hue" => out.sort_by(|a, b| hue_key(a.0).partial_cmp(&hue_key(b.0)).unwrap_or(std::cmp::Ordering::Equal)),
        "lightness" => out.sort_by(|a, b| luma(b.0).partial_cmp(&luma(a.0)).unwrap_or(std::cmp::Ordering::Equal)),
        _ => {}
    }
    Ok(out)
}

fn luma(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Hue in degrees, with near-greys sorted after the colours (by lightness) so a hue sort reads as
/// a rainbow followed by greys.
fn hue_key(c: [f32; 3]) -> f32 {
    let (mx, mn) = (c[0].max(c[1]).max(c[2]), c[0].min(c[1]).min(c[2]));
    let d = mx - mn;
    if d < 0.08 {
        return 400.0 + (1.0 - luma(c));
    }
    let h = if mx == c[0] { ((c[1] - c[2]) / d).rem_euclid(6.0) } else if mx == c[1] { (c[2] - c[0]) / d + 2.0 } else { (c[0] - c[1]) / d + 4.0 };
    h * 60.0
}

// ------------------------------------------------------------------ commands

fn group_by_name<'a>(groups: &'a [Group<Swatch>], name: Option<&str>, cmd: &str) -> Result<&'a Group<Swatch>> {
    match name {
        Some(n) => groups.iter().find(|g| g.name == n).ok_or_else(|| bad(cmd, format!("no swatch group \"{n}\""))),
        None => groups.first().ok_or_else(|| bad(cmd, "there are no swatch groups")),
    }
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    let groups: Vec<Value> = s.presets.swatches.iter().map(|g| json!({"name": g.name, "swatches": g.items.iter().map(Swatch::to_json).collect::<Vec<_>>()})).collect();
    Ok(json!({"groups": groups}))
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "swatch.presets.add";
    let color = match p.get("color") {
        None | Some(Value::Null) => {
            let f = s.tools.foreground;
            [f[0].clamp(0.0, 1.0), f[1].clamp(0.0, 1.0), f[2].clamp(0.0, 1.0)]
        }
        Some(v) => parse_color(v).ok_or_else(|| bad(CMD, "`color` must be \"#rrggbb\" or [r, g, b] in 0..1"))?,
    };
    let mut sw = Swatch { name: String::new(), color };
    let base = str_param(p, "name").map(str::to_string).unwrap_or_else(|| sw.hex());
    sw.name = unique_name(&s.presets.swatches, &base);
    let gi = group_index(&mut s.presets.swatches, str_param(p, "group"));
    let Some(group) = s.presets.swatches.get_mut(gi) else { return Err(EngineError::Other("no swatch group".into())) };
    group.items.push(sw.clone());
    let group = group.name.clone();
    s.presets_changed();
    Ok(json!({"name": sw.name, "color": sw.hex(), "group": group}))
}

fn apply(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "swatch.presets.apply";
    let name = req_str(p, "preset", CMD)?;
    let (gi, ii) = find(&s.presets.swatches, name, str_param(p, "group")).ok_or_else(|| bad(CMD, format!("no swatch \"{name}\" (see swatch.presets.list)")))?;
    let c = s.presets.swatches.get(gi).and_then(|g| g.items.get(ii)).map(|x| x.color).ok_or_else(|| bad(CMD, "no such swatch"))?;
    let target = match str_param(p, "target").unwrap_or("foreground") {
        t @ ("foreground" | "background") => t,
        other => return Err(bad(CMD, format!("`target` must be foreground or background, got {other}"))),
    };
    let rgba = [c[0], c[1], c[2], 1.0];
    if target == "foreground" {
        s.tools.foreground = rgba;
    } else {
        s.tools.background = rgba;
    }
    Ok(json!({"target": target, "color": hex_of(c)}))
}

fn edit(s: &mut Session, p: &Value) -> Result<Value> {
    let action = req_str(p, "action", "swatch.presets.edit")?.to_string();
    let r = edit_groups(&mut s.presets.swatches, &action, p, "swatch.presets.edit")?;
    s.presets_changed();
    Ok(r)
}

fn reset(s: &mut Session, p: &Value) -> Result<Value> {
    let append = p.get("append").and_then(Value::as_bool).unwrap_or(false);
    let mut b = builtin();
    if append {
        for g in &mut b {
            g.name = super::gradients::unique_group(&s.presets.swatches, &g.name);
        }
        s.presets.swatches.extend(b);
    } else {
        s.presets.swatches = b;
    }
    s.presets_changed();
    Ok(json!({"groups": s.presets.swatches.len()}))
}

fn from_image(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "swatch.presets.fromImage";
    let count = match p.get("count") {
        None | Some(Value::Null) => 8,
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(|x| x.round().clamp(2.0, 64.0) as usize).ok_or_else(|| bad(CMD, "`count` must be a number from 2 to 64"))?,
    };
    let sort = match str_param(p, "sort").unwrap_or("prominence") {
        t @ ("prominence" | "hue" | "lightness") => t,
        other => return Err(bad(CMD, format!("`sort` must be prominence, hue or lightness, got {other}"))),
    };
    let colors = prominent_colors(s, count, sort)?;
    let doc_name = s.active().map(|d| d.doc.name.clone()).unwrap_or_default();
    let base = str_param(p, "name").map(str::to_string).unwrap_or_else(|| if doc_name.is_empty() { "Palette".into() } else { format!("Palette of {doc_name}") });
    let name = super::gradients::unique_group(&s.presets.swatches, &base);
    let items: Vec<Swatch> = colors.iter().map(|(c, _)| Swatch { name: hex_of(*c), color: *c }).collect();
    s.presets.swatches.push(Group::new(&name, items));
    s.presets_changed();
    let list: Vec<Value> = colors.iter().map(|(c, share)| json!({"color": hex_of(*c), "share": (f64::from(*share) * 1000.0).round() / 1000.0})).collect();
    Ok(json!({"group": name, "colors": list}))
}

fn import(s: &mut Session, p: &Value, cmd: &str, ase: bool) -> Result<Value> {
    let (bytes, stem) = crate::preset_import_cmds::file_bytes(p, cmd, if ase { "an .ase file" } else { "an .aco file" })?;
    let parsed = if ase { files::parse_ase(&bytes) } else { files::parse_aco(&bytes) }.map_err(|e| EngineError::Other(format!("not a readable swatch file: {e}")))?;
    let mut groups = from_file(&parsed);
    if groups.is_empty() {
        return Err(EngineError::Other(if parsed.skipped > 0 { "the file only holds spot colours, which are not supported".into() } else { "the file holds no swatches".into() }));
    }
    let fallback = str_param(p, "group").map(str::to_string).or((!stem.is_empty()).then_some(stem)).unwrap_or_else(|| "Imported".into());
    let mut added = 0;
    for g in &mut groups {
        if g.name.is_empty() {
            g.name = fallback.clone();
        }
        g.name = super::gradients::unique_group(&s.presets.swatches, &g.name);
        added += g.items.len();
        s.presets.swatches.push(g.clone());
    }
    s.presets_changed();
    Ok(json!({"groups": groups.iter().map(|g| g.name.clone()).collect::<Vec<_>>(), "swatches": added, "skipped": parsed.skipped}))
}

fn import_aco(s: &mut Session, p: &Value) -> Result<Value> {
    import(s, p, "swatch.presets.importAco", false)
}

fn import_ase(s: &mut Session, p: &Value) -> Result<Value> {
    import(s, p, "swatch.presets.importAse", true)
}

/// Write the chosen group (or every group) to `path`, or return the file as base64 `data` when no
/// path is given (the browser build offers it as a download).
fn export(s: &mut Session, p: &Value, cmd: &str, ase: bool) -> Result<Value> {
    let groups: Vec<&Group<Swatch>> = match str_param(p, "group") {
        Some(_) => vec![group_by_name(&s.presets.swatches, str_param(p, "group"), cmd)?],
        None => s.presets.swatches.iter().collect(),
    };
    if groups.iter().all(|g| g.items.is_empty()) {
        return Err(EngineError::Other("there are no swatches to export".into()));
    }
    let file = to_file(&groups);
    let bytes = if ase {
        files::write_ase(&file)
    } else {
        // ACO has no groups: one flat list.
        files::write_aco(&file.iter().flat_map(|g| g.swatches.clone()).collect::<Vec<_>>())
    };
    let ext = if ase { "ase" } else { "aco" };
    let stem = groups.first().filter(|_| groups.len() == 1).map(|g| g.name.clone()).unwrap_or_else(|| "Swatches".into());
    match str_param(p, "path") {
        Some(path) => {
            std::fs::write(path, &bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
            Ok(json!({"path": path, "bytes": bytes.len(), "swatches": groups.iter().map(|g| g.items.len()).sum::<usize>()}))
        }
        None => Ok(json!({"fileName": format!("{stem}.{ext}"), "data": pixelwright_paint::tile::b64_encode(&bytes), "bytes": bytes.len()})),
    }
}

fn export_aco(s: &mut Session, p: &Value) -> Result<Value> {
    export(s, p, "swatch.presets.exportAco", false)
}

fn export_ase(s: &mut Session, p: &Value) -> Result<Value> {
    export(s, p, "swatch.presets.exportAse", true)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "swatch.presets.list",
            label: "Swatches",
            menu: &[],
            shortcut: None,
            params: "{} → {groups:[{name,swatches:[{name,color:\"#rrggbb\"}]}]}",
            enabled: always,
            run: list,
            journal: false,
        },
        CommandSpec {
            id: "swatch.presets.add",
            label: "New Swatch",
            menu: &[],
            shortcut: None,
            params: r##"{"color":"#rrggbb"|[r,g,b 0..1]=foreground,"name":str?=hex,"group":name?=first} → {name,color,group}"##,
            enabled: always,
            run: add,
            journal: true,
        },
        CommandSpec {
            id: "swatch.presets.apply",
            label: "Use Swatch",
            menu: &[],
            shortcut: None,
            params: r##"{"preset":name,"group":name?,"target":"foreground|background"="foreground"} → {target,color}"##,
            enabled: always,
            run: apply,
            journal: true,
        },
        CommandSpec {
            id: "swatch.presets.edit",
            label: "Edit Swatches",
            menu: &[],
            shortcut: None,
            params: super::GROUP_EDIT_PARAMS,
            enabled: always,
            run: edit,
            journal: true,
        },
        CommandSpec {
            id: "swatch.presets.reset",
            label: "Restore Default Swatches",
            menu: &[],
            shortcut: None,
            params: r##"{"append":bool=false}"##,
            enabled: always,
            run: reset,
            journal: true,
        },
        CommandSpec {
            id: "swatch.presets.fromImage",
            label: "Palette from Image",
            menu: &[],
            shortcut: None,
            params: r##"{"count":2..64=8,"sort":"prominence|hue|lightness"="prominence","name":str?} → {group,colors:[{color,share}]} (a new group of the image's most prominent colours)"##,
            enabled: super::has_doc,
            run: from_image,
            journal: true,
        },
        CommandSpec {
            id: "swatch.presets.importAco",
            label: "Import Swatches (ACO)",
            menu: &[],
            shortcut: None,
            params: r##"{"path":".aco"|"data":base64,"group":name?} → {groups,swatches,skipped}"##,
            enabled: always,
            run: import_aco,
            journal: true,
        },
        CommandSpec {
            id: "swatch.presets.importAse",
            label: "Import Swatches (ASE)",
            menu: &[],
            shortcut: None,
            params: r##"{"path":".ase"|"data":base64,"group":name?} → {groups,swatches,skipped}"##,
            enabled: always,
            run: import_ase,
            journal: true,
        },
        CommandSpec {
            id: "swatch.presets.exportAco",
            label: "Export Swatches (ACO)",
            menu: &[],
            shortcut: None,
            params: r##"{"group":name?=all,"path":str?} → {path,bytes,swatches} or, without path, {fileName,data:base64,bytes}"##,
            enabled: always,
            run: export_aco,
            journal: false,
        },
        CommandSpec {
            id: "swatch.presets.exportAse",
            label: "Export Swatches (ASE)",
            menu: &[],
            shortcut: None,
            params: r##"{"group":name?=all,"path":str?} → {path,bytes,swatches} or, without path, {fileName,data:base64,bytes}"##,
            enabled: always,
            run: export_ase,
            journal: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(s: &Session) -> Vec<String> {
        s.presets.swatches.iter().flat_map(|g| g.items.iter().map(|i| i.name.clone())).collect()
    }

    #[test]
    fn the_default_group_keeps_the_forty_colours_the_panel_had() {
        let s = Session::new();
        assert_eq!(s.presets.swatches.len(), 1);
        assert_eq!(s.presets.swatches[0].name, "Default");
        assert_eq!(s.presets.swatches[0].items.len(), 40);
        assert_eq!(s.presets.swatches[0].items[0].hex(), "#000000");
        assert_eq!(s.presets.swatches[0].items[9].hex(), "#FFFFFF");
        let n = names(&s);
        assert_eq!(n.len(), n.iter().collect::<std::collections::BTreeSet<_>>().len(), "names are unique");
    }

    #[test]
    fn add_apply_and_edit_work_by_name() {
        let mut s = Session::new();
        s.tools.foreground = [1.0, 0.5, 0.0, 1.0];
        let r = s.execute("swatch.presets.add", json!({"name": "Sunrise", "group": "Mine"})).unwrap();
        assert_eq!((r["name"].as_str(), r["color"].as_str(), r["group"].as_str()), (Some("Sunrise"), Some("#FF8000"), Some("Mine")));
        // A second one with the same name gets a unique one.
        let r2 = s.execute("swatch.presets.add", json!({"name": "Sunrise", "color": "#0af", "group": "Mine"})).unwrap();
        assert_eq!(r2["name"], "Sunrise 2");
        assert_eq!(r2["color"], "#00AAFF");
        s.execute("swatch.presets.apply", json!({"preset": "Sunrise 2", "target": "background"})).unwrap();
        assert_eq!(s.tools.background[..3], [0.0, 170.0 / 255.0, 1.0]);
        s.execute("swatch.presets.edit", json!({"action": "rename", "preset": "Sunrise", "name": "Sunset"})).unwrap();
        s.execute("swatch.presets.edit", json!({"action": "delete", "preset": "Sunrise 2"})).unwrap();
        let l = s.execute("swatch.presets.list", json!({})).unwrap();
        let mine = l["groups"].as_array().unwrap().iter().find(|g| g["name"] == "Mine").unwrap();
        assert_eq!(mine["swatches"].as_array().unwrap().len(), 1);
        assert_eq!(mine["swatches"][0]["name"], "Sunset");
        s.execute("swatch.presets.reset", json!({})).unwrap();
        assert_eq!(s.presets.swatches.len(), 1, "reset restores the default group only");
    }

    #[test]
    fn bad_parameters_are_errors_and_change_nothing() {
        let mut s = Session::new();
        let before = s.presets.swatches.clone();
        for (id, p) in [
            ("swatch.presets.add", json!({"color": "nope"})),
            ("swatch.presets.add", json!({"color": [1, 2]})),
            ("swatch.presets.add", json!({"color": 7})),
            ("swatch.presets.apply", json!({})),
            ("swatch.presets.apply", json!({"preset": "No such"})),
            ("swatch.presets.apply", json!({"preset": "Black", "target": "middle"})),
            ("swatch.presets.edit", json!({"action": "bogus"})),
            ("swatch.presets.importAco", json!({})),
            ("swatch.presets.importAco", json!({"data": "!!!"})),
            ("swatch.presets.importAse", json!({"data": "QUJD"})),
            ("swatch.presets.exportAco", json!({"group": "Missing"})),
            ("swatch.presets.fromImage", json!({})),
        ] {
            assert!(s.execute(id, p.clone()).is_err(), "{id} {p}");
        }
        assert_eq!(s.presets.swatches, before);
    }

    #[test]
    fn aco_and_ase_round_trip_through_export_and_import() {
        let mut s = Session::new();
        for (export, import) in [("swatch.presets.exportAco", "swatch.presets.importAco"), ("swatch.presets.exportAse", "swatch.presets.importAse")] {
            let mut t = Session::new();
            let out = s.execute(export, json!({"group": "Default"})).unwrap();
            assert!(out["fileName"].as_str().unwrap().starts_with("Default."));
            let r = t.execute(import, json!({"data": out["data"], "group": "Back"})).unwrap();
            assert_eq!(r["swatches"], 40, "{import}");
            let imported = t.presets.swatches.last().unwrap();
            assert!(imported.name.starts_with("Back") || imported.name.starts_with("Default"), "{}", imported.name);
            for (a, b) in imported.items.iter().zip(&s.presets.swatches[0].items) {
                assert_eq!(a.name, b.name);
                assert!(a.color.iter().zip(b.color).all(|(x, y)| (x - y).abs() < 0.01), "{} {:?} {:?}", a.name, a.color, b.color);
            }
        }
    }

    #[test]
    fn writing_to_a_path_and_reading_it_back_works() {
        let dir = std::env::temp_dir().join(format!("pixelwright-swatches-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("set.aco");
        let mut s = Session::new();
        s.execute("swatch.presets.exportAco", json!({"group": "Default", "path": path.to_str().unwrap()})).unwrap();
        let r = s.execute("swatch.presets.importAco", json!({"path": path.to_str().unwrap()})).unwrap();
        assert_eq!(r["swatches"], 40);
        assert_eq!(s.presets.swatches.last().unwrap().name, "set", "named after the file");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn palette_from_image_finds_the_colours_that_are_there() {
        for depth in [8, 16, 32] {
            let mut s = Session::new();
            s.execute("file.new", json!({"width": 64, "height": 64, "depth": depth, "background": "#ff0000"})).unwrap();
            s.execute("select.rect", json!({"x": 0, "y": 0, "width": 64, "height": 16})).unwrap();
            s.execute("edit.fill", json!({"color": "#0000ff"})).unwrap();
            s.execute("select.deselect", json!({})).unwrap();
            let r = s.execute("swatch.presets.fromImage", json!({"count": 4})).unwrap();
            let colors = r["colors"].as_array().unwrap();
            assert_eq!(colors.len(), 2, "{depth}-bit: just red and blue");
            assert_eq!(colors[0]["color"], "#FF0000", "{depth}-bit: red covers 75 %");
            assert!((colors[0]["share"].as_f64().unwrap() - 0.75).abs() < 0.05);
            assert_eq!(colors[1]["color"], "#0000FF");
            let g = s.presets.swatches.iter().find(|g| g.name == r["group"].as_str().unwrap()).unwrap();
            assert_eq!(g.items.len(), 2);
            let by_hue = s.execute("swatch.presets.fromImage", json!({"count": 4, "sort": "hue"})).unwrap();
            assert_eq!(by_hue["colors"][0]["color"], "#FF0000", "red comes before blue by hue");
        }
    }

    #[test]
    fn persisted_swatches_survive_a_restart() {
        let mut s = Session::new();
        s.execute("swatch.presets.add", json!({"name": "Keep", "color": "#123456", "group": "Mine"})).unwrap();
        let saved = s.presets.to_json(&s);
        let mut t = Session::new();
        t.load_presets_json(saved);
        assert!(t.presets.swatches.iter().any(|g| g.name == "Mine" && g.items.iter().any(|i| i.name == "Keep" && i.hex() == "#123456")));
        // An old preferences blob without swatches leaves the defaults.
        let mut u = Session::new();
        u.load_presets_json(json!({"gradients": null}));
        assert_eq!(u.presets.swatches[0].name, "Default");
    }

    #[test]
    fn colour_models_convert_to_srgb() {
        assert_eq!(to_srgb(&SwatchColor::Cmyk([0.0, 0.0, 0.0, 1.0])), [0.0, 0.0, 0.0]);
        assert_eq!(to_srgb(&SwatchColor::Cmyk([0.0, 0.0, 0.0, 0.0])), [1.0, 1.0, 1.0]);
        assert_eq!(to_srgb(&SwatchColor::Gray(0.5)), [0.5, 0.5, 0.5]);
        assert_eq!(to_srgb(&SwatchColor::Hsb([240.0, 1.0, 1.0])), [0.0, 0.0, 1.0]);
        let white = to_srgb(&SwatchColor::Lab([100.0, 0.0, 0.0]));
        assert!(white.iter().all(|v| *v > 0.98), "{white:?}");
        assert_eq!(to_srgb(&SwatchColor::Rgb([f32::NAN, 2.0, -1.0])), [0.0, 1.0, 0.0], "never non-finite or out of range");
    }
}
