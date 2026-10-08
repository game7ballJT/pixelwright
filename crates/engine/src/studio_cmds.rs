//! Studio commands: finishing and retouching moves that modern editors offer and that the
//! upstream PhotoCraft command set did not have. Every one is a registry command, so the menus,
//! the command palette, the CLI, the control channel and MCP all reach it the same way.
//!
//! - **Frequency Separation** (`layer.frequencySeparation`): splits a pixel layer into a blurred
//!   *low* layer (colour and tone) and a Linear Light *high* layer (texture), the standard
//!   non-destructive retouching setup. Stacked, the two reproduce the original exactly (the high
//!   layer stores `(source − low) / 2 + ½`, which Linear Light doubles back), and the original is
//!   hidden, not removed.
//! - **Dodge & Burn Layer** (`layer.newDodgeBurn`): a 50 % grey Overlay (or Soft Light) layer to
//!   paint light and shadow onto without touching pixels.
//! - **Orton Glow** (`layer.ortonGlow`): a blurred copy in Screen mode; its opacity is the amount,
//!   so the glow stays adjustable afterwards.
//! - **Pixel Art** (`filter.pixelate.pixelArt`): averages cells, reduces them to a small palette
//!   (the Indexed Color palette builders and dithers) and scales the cells back up.
//! - **Straighten Horizon** (`image.autoStraighten`): finds the dominant near-horizontal or
//!   near-vertical edge direction from the composite and rotates and crops by it, as the Ruler
//!   Straighten does; `"apply": false` only measures.
//!
//! All of them work in the document's own colour model and depth (through the RGBA conversions of
//! `pixelwright-raster`), never in a `u8` copy.

use pixelwright_algo::FilterParams;
use pixelwright_algo::quantize::{self, Dither, Forced, PaletteKind};
use pixelwright_color::{BlendMode, PixelFormat};
use pixelwright_doc::{Document, Layer, LayerContent};
use pixelwright_geom::Rect;
use pixelwright_raster::{Surface, from_rgba_into, is_hdr, to_rgba};
use serde_json::{Value, json};

use crate::commands::{CommandSpec, layer_param};
use crate::{EngineError, Result, Session};

fn bad(cmd: &str, msg: impl Into<String>) -> EngineError {
    EngineError::BadParams { cmd: cmd.into(), msg: msg.into() }
}

fn has_doc(s: &Session) -> std::result::Result<(), String> {
    s.active().map(|_| ()).ok_or_else(|| "no document open".into())
}

fn has_pixels(s: &Session) -> std::result::Result<(), String> {
    let l = crate::active_layer_of(s)?;
    match l.content {
        LayerContent::Raster(_) => Ok(()),
        ref c => Err(format!("active layer is a {} layer, not a pixel layer", c.kind_name())),
    }
}

fn object(p: &Value, cmd: &str) -> Result<()> {
    match p {
        Value::Object(_) | Value::Null => Ok(()),
        _ => Err(bad(cmd, "expected an object")),
    }
}

/// A number parameter: `default` when absent, an error when present but not a finite number,
/// clamped into `lo..=hi`.
fn number(p: &Value, key: &str, default: f64, lo: f64, hi: f64, cmd: &str) -> Result<f64> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v.as_f64().filter(|x| x.is_finite()).map(|x| x.clamp(lo, hi)).ok_or_else(|| bad(cmd, format!("`{key}` must be a number"))),
    }
}

/// A string parameter restricted to `allowed` (the first entry is not special; `default` is).
fn choice<'a>(p: &'a Value, key: &str, default: &'a str, allowed: &[&str], cmd: &str) -> Result<&'a str> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::String(v)) if allowed.contains(&v.as_str()) => Ok(v.as_str()),
        Some(_) => Err(bad(cmd, format!("`{key}` must be one of: {}", allowed.join(", ")))),
    }
}

/// `area` of `surf` as RGBA, converted from whatever the layer's mode and depth are.
fn read_rgba(surf: &Surface, area: Rect) -> Vec<[f32; 4]> {
    let fmt = surf.format();
    let n = fmt.channels();
    surf.read_region(area).chunks_exact(n).map(|p| to_rgba(&fmt, p)).collect()
}

/// Write RGBA pixels (row-major over `area`) into a new surface in `fmt`.
fn surface_from_rgba(fmt: PixelFormat, area: Rect, px: &[[f32; 4]]) -> Surface {
    let n = fmt.channels();
    let mut raw = vec![0.0f32; px.len() * n];
    let mut enc = [0.0f32; 8];
    for (p, out) in px.iter().zip(raw.chunks_exact_mut(n)) {
        let k = from_rgba_into(&fmt, *p, &mut enc).min(n);
        if let Some(o) = out.get_mut(..k) {
            o.copy_from_slice(&enc[..k]);
        }
    }
    let mut surf = Surface::new(fmt);
    surf.write_region(area, &raw);
    surf.prune();
    surf
}

/// The canvas plus whatever the layer holds beyond it.
fn extent_of(doc: &Document, surf: &Surface) -> Rect {
    doc.bounds().union(&surf.content_bounds())
}

fn gaussian(surf: &Surface, doc: &Document, extent: Rect, radius: f32) -> Surface {
    pixelwright_algo::apply_in(surf, &FilterParams::GaussianBlur { radius }, extent, doc.bounds(), None, extent)
}

// ---------------------------------------------------------------------------------------------
// Frequency Separation

fn frequency_separation(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layer.frequencySeparation";
    object(p, CMD)?;
    let radius = number(p, "radius", 6.0, 0.5, 250.0, CMD)? as f32;
    let id = layer_param(s, p)?;
    let (group, low, high) = s.edit("Frequency Separation", |doc, active| {
        let src = doc.layer(id).ok_or(EngineError::NoLayer(id))?;
        let surf = src.surface().ok_or_else(|| EngineError::Other("not a pixel layer".into()))?;
        let extent = extent_of(doc, surf);
        if extent.is_empty() {
            return Err(EngineError::Other("the layer is empty".into()));
        }
        let name = src.name.clone();
        let fmt = surf.format();
        let low_surf = gaussian(surf, doc, extent, radius);
        let (a, b) = (read_rgba(surf, extent), read_rgba(&low_surf, extent));
        let hdr = is_hdr(fmt.sample);
        // Floating-point documents keep out-of-range detail; integer ones clamp.
        let high_px: Vec<[f32; 4]> = a
            .iter()
            .zip(&b)
            .map(|(s, l)| {
                let mut h = [0.0f32; 4];
                for c in 0..3 {
                    let v = (s[c] - l[c]) * 0.5 + 0.5;
                    h[c] = if hdr { v } else { v.clamp(0.0, 1.0) };
                }
                h[3] = s[3];
                h
            })
            .collect();
        let mut low_layer = Layer::raster(format!("{name} low"), fmt);
        *crate::pixels_mut(&mut low_layer)? = low_surf;
        let mut high_layer = Layer::raster(format!("{name} high"), fmt);
        *crate::pixels_mut(&mut high_layer)? = surface_from_rgba(fmt, extent, &high_px);
        high_layer.blend = BlendMode::LinearLight;
        let (low, high) = (low_layer.id, high_layer.id);
        let group = Layer::group(format!("Frequency Separation ({radius} px)"), vec![low_layer, high_layer]);
        let gid = group.id;
        doc.insert_above(Some(id), group);
        doc.layer_mut(id).ok_or(EngineError::NoLayer(id))?.visible = false;
        *active = Some(high);
        Ok((gid, low, high))
    })?;
    Ok(json!({ "group": group.0, "low": low.0, "high": high.0, "radius": radius }))
}

// ---------------------------------------------------------------------------------------------
// Dodge & Burn layer

fn new_dodge_burn(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layer.newDodgeBurn";
    object(p, CMD)?;
    let mode = match choice(p, "mode", "overlay", &["overlay", "softLight"], CMD)? {
        "softLight" => BlendMode::SoftLight,
        _ => BlendMode::Overlay,
    };
    let id = s.edit("New Dodge & Burn Layer", |doc, active| {
        let canvas = doc.bounds();
        let f = doc.pixel_format();
        let fmt = PixelFormat::new(f.mode, f.sample, true);
        let mut layer = Layer::raster(doc.next_layer_name("Dodge & Burn"), fmt);
        let mut grey = [0.0f32; 8];
        let n = from_rgba_into(&fmt, [0.5, 0.5, 0.5, 1.0], &mut grey).min(fmt.channels());
        crate::pixels_mut(&mut layer)?.fill_rect(canvas, &grey[..n]);
        layer.blend = mode;
        let id = doc.insert_above(*active, layer);
        *active = Some(id);
        Ok(id)
    })?;
    Ok(json!({ "layer": id.0 }))
}

// ---------------------------------------------------------------------------------------------
// Orton glow

fn orton_glow(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "layer.ortonGlow";
    object(p, CMD)?;
    let radius = number(p, "radius", 20.0, 1.0, 250.0, CMD)? as f32;
    let amount = number(p, "amount", 60.0, 0.0, 100.0, CMD)?;
    let id = layer_param(s, p)?;
    let glow = s.edit("Orton Glow", |doc, active| {
        let src = doc.layer(id).ok_or(EngineError::NoLayer(id))?;
        let surf = src.surface().ok_or_else(|| EngineError::Other("not a pixel layer".into()))?;
        let extent = extent_of(doc, surf);
        if extent.is_empty() {
            return Err(EngineError::Other("the layer is empty".into()));
        }
        let mut layer = Layer::raster(format!("{} glow", src.name), surf.format());
        *crate::pixels_mut(&mut layer)? = gaussian(surf, doc, extent, radius);
        layer.blend = BlendMode::Screen;
        layer.opacity = (amount / 100.0) as f32;
        let gid = doc.insert_above(Some(id), layer);
        *active = Some(gid);
        Ok(gid)
    })?;
    Ok(json!({ "layer": glow.0, "radius": radius, "amount": amount }))
}

// ---------------------------------------------------------------------------------------------
// Pixel Art

/// Average `cell` × `cell` blocks of `px` (an `area`-sized RGBA grid), reduce the cells to
/// `colors` colours with `dither`, and paint each cell's colour back over its block.
fn pixelize(area: Rect, px: &mut [[f32; 4]], cell: usize, colors: usize, kind: PaletteKind, dither: Dither) {
    let (w, h) = (area.width() as usize, area.height() as usize);
    if w == 0 || h == 0 || cell == 0 || px.len() != w * h {
        return;
    }
    let (gw, gh) = (w.div_ceil(cell), h.div_ceil(cell));
    let mut grid = vec![[0.0f32; 4]; gw * gh];
    for gy in 0..gh {
        for gx in 0..gw {
            let (x1, y1) = (((gx + 1) * cell).min(w), ((gy + 1) * cell).min(h));
            let (mut sum, mut alpha, mut n) = ([0.0f32; 3], 0.0f32, 0.0f32);
            for y in gy * cell..y1 {
                for x in gx * cell..x1 {
                    let Some(q) = px.get(y * w + x) else { continue };
                    for c in 0..3 {
                        sum[c] += q[c] * q[3];
                    }
                    alpha += q[3];
                    n += 1.0;
                }
            }
            if let Some(g) = grid.get_mut(gy * gw + gx)
                && alpha > 0.0
                && n > 0.0
            {
                *g = [sum[0] / alpha, sum[1] / alpha, sum[2] / alpha, alpha / n];
            }
        }
    }
    if let Ok(pal) = quantize::build_palette(&grid, kind, colors, Forced::None) {
        quantize::quantize(&mut grid, gw, &pal, dither, 0.6, None);
    }
    for y in 0..h {
        for x in 0..w {
            if let (Some(out), Some(g)) = (px.get_mut(y * w + x), grid.get((y / cell) * gw + x / cell)) {
                *out = *g;
            }
        }
    }
}

fn pixel_art(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "filter.pixelate.pixelArt";
    object(p, CMD)?;
    let cell = number(p, "cellSize", 6.0, 1.0, 128.0, CMD)?.round() as usize;
    let colors = number(p, "colors", 16.0, 2.0, 256.0, CMD)?.round() as usize;
    let dither = match choice(p, "dither", "pattern", &["none", "pattern", "diffusion", "noise"], CMD)? {
        "none" => Dither::None,
        "diffusion" => Dither::Diffusion,
        "noise" => Dither::Noise,
        _ => Dither::Pattern,
    };
    let kind = match choice(p, "palette", "perceptual", &["perceptual", "adaptive"], CMD)? {
        "adaptive" => PaletteKind::Adaptive,
        _ => PaletteKind::Perceptual,
    };
    crate::extra_cmds::map_pixels(s, "Pixel Art", move |area, px, _| pixelize(area, px, cell, colors, kind, dither))?;
    Ok(json!({ "cellSize": cell, "colors": colors }))
}

// ---------------------------------------------------------------------------------------------
// Straighten Horizon

/// Tilt of the dominant axis-aligned edges in a grey image, in degrees clockwise (screen sense,
/// y down: a line sloping down to the right is positive), with a confidence in 0..=1 (the share of
/// edge strength agreeing with it). `None` when there are no usable edges.
///
/// The image is smoothed first (two binomial passes, about 2 px), so hard stair-stepped edges
/// vote with their true slope instead of piling up at 0°. Sobel gradients then vote, each pixel
/// casting its edge direction's deviation from the nearest axis (−45°..45°) weighted by gradient
/// strength; only gradients within a tenth of the strongest count. The vote histogram is smoothed
/// over ±2°, and the centre of its highest plateau inside ±`max_angle` wins, refined by the
/// vote-weighted mean within ±1.5°. Horizontal and vertical edges both vote, so a building or a
/// horizon works alike.
pub(crate) fn tilt_degrees(grey: &[f32], w: usize, h: usize, max_angle: f64) -> Option<(f64, f64)> {
    if w < 8 || h < 8 || grey.len() != w * h {
        return None;
    }
    const BINS: usize = 360; // 0.25° each over −45°..45°
    let g = smooth_grey(grey, w, h);
    let at = |x: usize, y: usize| g.get(y * w + x).copied().unwrap_or(0.0) as f64;
    // (deviation from the nearest axis in degrees, gradient magnitude) per interior pixel.
    let mut votes: Vec<(f64, f64)> = Vec::with_capacity((w - 2) * (h - 2));
    let mut strongest = 0.0f64;
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let gx = (at(x + 1, y - 1) + 2.0 * at(x + 1, y) + at(x + 1, y + 1)) - (at(x - 1, y - 1) + 2.0 * at(x - 1, y) + at(x - 1, y + 1));
            let gy = (at(x - 1, y + 1) + 2.0 * at(x, y + 1) + at(x + 1, y + 1)) - (at(x - 1, y - 1) + 2.0 * at(x, y - 1) + at(x + 1, y - 1));
            let m = gx.hypot(gy);
            if !m.is_finite() {
                continue;
            }
            strongest = strongest.max(m);
            // The edge runs perpendicular to the gradient; fold its direction onto ±45° of an axis.
            let mut d = (gy.atan2(gx).to_degrees() + 90.0) % 90.0;
            if d > 45.0 {
                d -= 90.0;
            } else if d < -45.0 {
                d += 90.0;
            }
            votes.push((d, m));
        }
    }
    if strongest < 1e-3 {
        return None;
    }
    let floor = strongest * 0.1;
    let mut hist = vec![0.0f64; BINS];
    let mut total = 0.0f64;
    for &(d, m) in &votes {
        if m < floor {
            continue;
        }
        let bin = (((d + 45.0) / 0.25) as usize).min(BINS - 1);
        if let Some(b) = hist.get_mut(bin) {
            *b += m;
        }
        total += m;
    }
    if total <= 0.0 {
        return None;
    }
    // Smooth over ±8 bins (±2°); the highest plateau inside the allowed range, at its centre.
    let smooth: Vec<f64> = (0..BINS).map(|i| (i.saturating_sub(8)..(i + 9).min(BINS)).map(|j| hist.get(j).copied().unwrap_or(0.0)).sum()).collect();
    let lim = (max_angle.clamp(0.25, 45.0) / 0.25) as usize;
    let (lo, hi) = ((BINS / 2).saturating_sub(lim), (BINS / 2 + lim).min(BINS - 1));
    let best = (lo..=hi).filter_map(|i| smooth.get(i).copied()).fold(0.0f64, f64::max);
    if best <= 0.0 {
        return None;
    }
    let plateau: Vec<usize> = (lo..=hi).filter(|i| smooth.get(*i).is_some_and(|v| *v >= best * 0.999)).collect();
    let peak = *plateau.get(plateau.len() / 2)?;
    let (mut num, mut den) = (0.0f64, 0.0f64);
    for i in peak.saturating_sub(6)..=(peak + 6).min(BINS - 1) {
        let v = hist.get(i).copied().unwrap_or(0.0);
        num += v * ((i as f64 + 0.5) * 0.25 - 45.0);
        den += v;
    }
    if den <= 0.0 {
        return None;
    }
    Some((num / den, (den / total).clamp(0.0, 1.0)))
}

/// Two passes of the binomial kernel [1 4 6 4 1] / 16 in both directions (edges clamped): a
/// Gaussian of about 2 px.
fn smooth_grey(grey: &[f32], w: usize, h: usize) -> Vec<f32> {
    const K: [f32; 5] = [1.0, 4.0, 6.0, 4.0, 1.0];
    let pass = |src: &[f32], horizontal: bool| -> Vec<f32> {
        let mut out = vec![0.0f32; src.len()];
        for y in 0..h {
            for x in 0..w {
                let mut acc = 0.0f32;
                for (k, weight) in K.iter().enumerate() {
                    let off = k as isize - 2;
                    let (sx, sy) = if horizontal {
                        ((x as isize + off).clamp(0, w as isize - 1) as usize, y)
                    } else {
                        (x, (y as isize + off).clamp(0, h as isize - 1) as usize)
                    };
                    acc += weight * src.get(sy * w + sx).copied().unwrap_or(0.0);
                }
                if let Some(o) = out.get_mut(y * w + x) {
                    *o = acc / 16.0;
                }
            }
        }
        out
    };
    let a = pass(grey, true);
    let b = pass(&a, false);
    let c = pass(&b, true);
    pass(&c, false)
}

/// The document's composite as a grey image no larger than `limit` pixels on its long side.
fn grey_thumbnail(doc: &Document, limit: usize) -> Option<(Vec<f32>, usize, usize)> {
    let f = doc.pixel_format();
    let comp = pixelwright_compose::flatten_to_surface(doc, PixelFormat::new(f.mode, f.sample, true), None);
    let b = doc.bounds();
    let (w, h) = (b.width() as usize, b.height() as usize);
    if w == 0 || h == 0 {
        return None;
    }
    let step = w.max(h).div_ceil(limit.max(8)).max(1);
    let (tw, th) = (w / step, h / step);
    let mut grey = Vec::with_capacity(tw * th);
    // Each thumbnail pixel averages up to 4 × 4 samples of its block, so edges stay anti-aliased
    // and keep their true slope (point sampling would turn a shallow line into stair steps).
    let taps = step.min(4);
    for ty in 0..th {
        for tx in 0..tw {
            let mut sum = 0.0f32;
            for j in 0..taps {
                for i in 0..taps {
                    let (sx, sy) = (tx * step + (2 * i + 1) * step / (2 * taps), ty * step + (2 * j + 1) * step / (2 * taps));
                    let px = comp.rgba(b.x0 + sx as i32, b.y0 + sy as i32);
                    // Transparent areas read as mid grey so they carry no edges of their own.
                    let a = px[3].clamp(0.0, 1.0);
                    let y = 0.2126 * px[0] + 0.7152 * px[1] + 0.0722 * px[2];
                    sum += y.clamp(0.0, 1.0) * a + 0.5 * (1.0 - a);
                }
            }
            grey.push(sum / (taps * taps) as f32);
        }
    }
    Some((grey, tw, th))
}

fn auto_straighten(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "image.autoStraighten";
    object(p, CMD)?;
    let max_angle = number(p, "maxAngle", 10.0, 0.5, 20.0, CMD)?;
    let min_conf = number(p, "minConfidence", 0.12, 0.0, 1.0, CMD)?;
    let apply = match p.get("apply") {
        None | Some(Value::Null) => true,
        Some(Value::Bool(b)) => *b,
        Some(_) => return Err(bad(CMD, "`apply` must be true or false")),
    };
    let crop = match p.get("crop") {
        None | Some(Value::Null) => None,
        Some(Value::Bool(b)) => Some(*b),
        Some(_) => return Err(bad(CMD, "`crop` must be true or false")),
    };
    let doc = s.active().ok_or(EngineError::NoDocument)?.doc.clone();
    let (grey, w, h) = grey_thumbnail(&doc, 640).ok_or_else(|| EngineError::Other("the document is empty".into()))?;
    let Some((tilt, confidence)) = tilt_degrees(&grey, w, h, max_angle) else {
        return Ok(json!({ "angle": 0.0, "confidence": 0.0, "applied": false, "reason": "no clear edges to measure" }));
    };
    let round = |v: f64| (v * 100.0).round() / 100.0;
    if confidence < min_conf {
        return Ok(json!({ "angle": 0.0, "confidence": round(confidence), "applied": false, "reason": "no dominant horizon or vertical edge" }));
    }
    // A line that slopes down to the right is levelled by turning the image counter-clockwise.
    let rotate_cw = -tilt;
    if rotate_cw.abs() < 0.15 || !apply {
        return Ok(json!({ "angle": round(rotate_cw), "confidence": round(confidence), "applied": false, "reason": if apply { "already level" } else { "measure only" } }));
    }
    let r = crate::analysis_cmds::straighten_by(s, rotate_cw, crop)?;
    Ok(json!({ "angle": round(rotate_cw), "confidence": round(confidence), "applied": true, "width": r["width"], "height": r["height"], "cropped": r["cropped"] }))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "layer.frequencySeparation",
            label: "Frequency Separation…",
            menu: &["Layer", "New"],
            shortcut: None,
            params: r##"{"radius":0.5..250=6,"layer":id} → {group, low, high} (the pixel layer split into a blurred Low layer and a Linear Light High layer in a group; the original is hidden)"##,
            enabled: has_pixels,
            journal: true,
            run: frequency_separation,
        },
        CommandSpec {
            id: "layer.newDodgeBurn",
            label: "Dodge & Burn Layer",
            menu: &["Layer", "New"],
            shortcut: None,
            params: r##"{"mode":"overlay|softLight"="overlay"} → {layer} (a 50 % grey layer above the active one, to paint light and shadow on)"##,
            enabled: has_doc,
            journal: true,
            run: new_dodge_burn,
        },
        CommandSpec {
            id: "layer.ortonGlow",
            label: "Orton Glow…",
            menu: &["Layer", "New"],
            shortcut: None,
            params: r##"{"radius":1..250=20,"amount":0..100=60,"layer":id} → {layer} (a blurred copy in Screen mode; its opacity is the amount)"##,
            enabled: has_pixels,
            journal: true,
            run: orton_glow,
        },
        CommandSpec {
            id: "filter.pixelate.pixelArt",
            label: "Pixel Art…",
            menu: &["Filter", "Pixelate"],
            shortcut: None,
            params: r##"{"cellSize":1..128=6,"colors":2..256=16,"dither":"none|pattern|diffusion|noise"="pattern","palette":"perceptual|adaptive"="perceptual"} (cells averaged, reduced to a small palette, scaled back up; honours the selection)"##,
            enabled: has_pixels,
            journal: true,
            run: pixel_art,
        },
        CommandSpec {
            id: "image.autoStraighten",
            label: "Straighten Horizon",
            menu: &["Image", "Image Rotation"],
            shortcut: None,
            params: r##"{"maxAngle":0.5..20=10,"minConfidence":0..1=0.12,"apply":bool=true,"crop":bool} → {angle (clockwise degrees), confidence, applied}"##,
            enabled: has_doc,
            journal: true,
            run: auto_straighten,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(depth: u32) -> Session {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 48, "height": 32, "depth": depth, "background": "#ffffff"})).unwrap();
        s
    }

    /// A photo-like layer: a dark ramp on the left, a bright square on the right.
    fn painted(depth: u32) -> Session {
        let mut s = session(depth);
        s.execute("select.rect", json!({"x": 24, "y": 8, "width": 16, "height": 16})).unwrap();
        s.execute("edit.fill", json!({"color": "#ff8000"})).unwrap();
        s.execute("select.deselect", json!({})).unwrap();
        s
    }

    fn rgba_of(s: &Session, id: pixelwright_doc::LayerId, x: i32, y: i32) -> [f32; 4] {
        s.active().unwrap().doc.layer(id).unwrap().surface().unwrap().rgba(x, y)
    }

    fn id_from(r: &Value, key: &str) -> pixelwright_doc::LayerId {
        pixelwright_doc::LayerId(r[key].as_u64().unwrap())
    }

    #[test]
    fn frequency_separation_stacks_back_to_the_original() {
        for depth in [8, 16, 32] {
            let mut s = painted(depth);
            let bg = s.active().unwrap().doc.layers[0].id;
            let before = (rgba_of(&s, bg, 30, 14), rgba_of(&s, bg, 4, 4), rgba_of(&s, bg, 23, 14));
            let r = s.execute("layer.frequencySeparation", json!({"radius": 2})).unwrap();
            let (low, high) = (id_from(&r, "low"), id_from(&r, "high"));
            let doc = &s.active().unwrap().doc;
            assert!(!doc.layer(bg).unwrap().visible, "the original is hidden");
            assert_eq!(doc.layer(high).unwrap().blend, BlendMode::LinearLight);
            assert_eq!(doc.layer(low).unwrap().blend, BlendMode::Normal);
            assert_eq!(s.active().unwrap().active_layer, Some(high));
            // Reconstruction: low + 2 * (high − ½) equals the source (Linear Light), off the edges
            // exactly and across the edge within quantisation.
            let tol = if depth == 8 { 2.5 / 255.0 } else { 1e-3 };
            for (x, y, want) in [(30, 14, before.0), (4, 4, before.1), (23, 14, before.2)] {
                let (l, h) = (rgba_of(&s, low, x, y), rgba_of(&s, high, x, y));
                for c in 0..3 {
                    let got = (l[c] + 2.0 * (h[c] - 0.5)).clamp(0.0, 1.0);
                    assert!((got - want[c]).abs() <= tol * 2.0, "{depth}-bit ({x},{y}) channel {c}: {got} vs {}", want[c]);
                }
            }
            // One undo step removes the group and shows the original again.
            s.undo();
            assert!(s.active().unwrap().doc.layer(bg).unwrap().visible);
            assert!(s.active().unwrap().doc.layer(low).is_none());
        }
    }

    #[test]
    fn frequency_separation_fails_gracefully() {
        let mut s = Session::new();
        assert!(s.execute("layer.frequencySeparation", json!({})).is_err(), "no document");
        let mut s = session(8);
        let rev = s.active().unwrap().revision;
        for bad in [json!([1]), json!("x"), json!({"radius": "wide"}), json!({"radius": [3]}), json!({"layer": 987654321})] {
            assert!(s.execute("layer.frequencySeparation", bad.clone()).is_err(), "{bad}");
        }
        assert_eq!(s.active().unwrap().revision, rev, "nothing changed");
        // Absurd radii are clamped, not trusted.
        assert!(s.execute("layer.frequencySeparation", json!({"radius": 1e12})).is_ok());
    }

    #[test]
    fn dodge_burn_layer_is_neutral_grey_in_overlay() {
        for depth in [8, 16, 32] {
            let mut s = session(depth);
            let r = s.execute("layer.newDodgeBurn", json!({})).unwrap();
            let id = id_from(&r, "layer");
            let l = s.active().unwrap().doc.layer(id).unwrap().clone();
            assert_eq!(l.blend, BlendMode::Overlay);
            assert!(l.name.starts_with("Dodge & Burn"), "{}", l.name);
            let p = rgba_of(&s, id, 10, 10);
            assert!(p[..3].iter().all(|v| (v - 0.5).abs() < 0.01) && p[3] > 0.99, "{depth}: {p:?}");
            // Overlay with 50 % grey leaves the image unchanged.
            let bg = s.active().unwrap().doc.layers[0].id;
            let comp = pixelwright_compose::flatten_to_surface(&s.active().unwrap().doc, s.active().unwrap().doc.pixel_format(), None);
            assert!(comp.rgba(10, 10).iter().zip(rgba_of(&s, bg, 10, 10)).all(|(a, b)| (a - b).abs() < 0.01));
        }
        let mut s = session(8);
        let r = s.execute("layer.newDodgeBurn", json!({"mode": "softLight"})).unwrap();
        assert_eq!(s.active().unwrap().doc.layer(id_from(&r, "layer")).unwrap().blend, BlendMode::SoftLight);
        assert!(s.execute("layer.newDodgeBurn", json!({"mode": "bogus"})).is_err());
        assert!(s.execute("layer.newDodgeBurn", json!({"mode": 3})).is_err());
    }

    #[test]
    fn orton_glow_adds_a_screen_layer_whose_opacity_is_the_amount() {
        let mut s = painted(8);
        let bg = s.active().unwrap().doc.layers[0].id;
        let r = s.execute("layer.ortonGlow", json!({"radius": 3, "amount": 40})).unwrap();
        let g = s.active().unwrap().doc.layer(id_from(&r, "layer")).unwrap().clone();
        assert_eq!(g.blend, BlendMode::Screen);
        assert!((g.opacity - 0.4).abs() < 1e-6);
        // The blur bleeds colour past the square's edge.
        assert!(rgba_of(&s, g.id, 22, 14)[2] < 0.9 || rgba_of(&s, g.id, 22, 14)[1] < 0.99);
        assert!(s.active().unwrap().doc.layer(bg).unwrap().visible, "the original stays visible");
        assert!(s.execute("layer.ortonGlow", json!({"amount": "lots"})).is_err());
    }

    #[test]
    fn pixel_art_snaps_cells_to_a_small_palette() {
        for depth in [8, 16, 32] {
            let mut s = painted(depth);
            s.execute("filter.pixelate.pixelArt", json!({"cellSize": 4, "colors": 2, "dither": "none"})).unwrap();
            let bg = s.active().unwrap().doc.layers[0].id;
            let mut seen = std::collections::BTreeSet::new();
            for y in 0..32 {
                for x in 0..48 {
                    let p = rgba_of(&s, bg, x, y);
                    seen.insert([(p[0] * 255.0).round() as i32, (p[1] * 255.0).round() as i32, (p[2] * 255.0).round() as i32]);
                    // Every 4 × 4 cell is one flat colour.
                    let q = rgba_of(&s, bg, x - x % 4, y - y % 4);
                    assert!(p.iter().zip(q).all(|(a, b)| (a - b).abs() < 1e-4), "{depth}-bit ({x},{y})");
                }
            }
            assert!(seen.len() <= 2, "{depth}-bit: {} colours", seen.len());
        }
        let mut s = painted(8);
        for bad in [json!({"dither": "magic"}), json!({"cellSize": "big"}), json!({"palette": 1}), json!([])] {
            assert!(s.execute("filter.pixelate.pixelArt", bad.clone()).is_err(), "{bad}");
        }
        assert!(s.execute("filter.pixelate.pixelArt", json!({"cellSize": 0, "colors": 9999})).is_ok(), "out-of-range numbers clamp");
    }

    /// Smooth stripes (a sine wave, as an anti-aliased photo edge would be) tilted by `deg`:
    /// horizontal stripes slope down to the right when positive; vertical ones are the same
    /// pattern turned a quarter, so they lean the opposite way for the same sign.
    fn stripes(w: usize, h: usize, deg: f64, vertical: bool) -> Vec<f32> {
        let t = deg.to_radians().tan();
        (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f64, (i / w) as f64);
                let v = if vertical { x - t * y } else { y - t * x };
                (0.5 + 0.4 * (std::f64::consts::TAU * v / 12.0).sin()) as f32
            })
            .collect()
    }

    #[test]
    fn tilt_is_measured_from_horizontal_and_vertical_edges() {
        for (deg, vertical) in [(3.0, false), (-4.5, false), (2.0, true), (-6.0, true), (0.0, false)] {
            let g = stripes(160, 120, deg, vertical);
            let (tilt, conf) = tilt_degrees(&g, 160, 120, 10.0).unwrap();
            // Vertical stripes drawn with x − t·y slope the other way on screen.
            let want = if vertical { -deg } else { deg };
            assert!((tilt - want).abs() < 0.8, "{deg}° {vertical}: measured {tilt}");
            assert!(conf > 0.3, "confidence {conf}");
        }
    }

    #[test]
    fn tilt_rejects_flat_and_malformed_input() {
        assert!(tilt_degrees(&vec![0.5; 100 * 100], 100, 100, 10.0).is_none(), "no edges");
        assert!(tilt_degrees(&[], 0, 0, 10.0).is_none());
        assert!(tilt_degrees(&[0.0; 10], 100, 100, 10.0).is_none(), "wrong length");
        assert!(tilt_degrees(&[0.0; 4], 2, 2, 10.0).is_none(), "too small");
        // NaN-ish limits never panic.
        let g = stripes(64, 64, 2.0, false);
        let _ = tilt_degrees(&g, 64, 64, f64::NAN);
    }

    #[test]
    fn auto_straighten_levels_a_tilted_document_and_can_only_measure() {
        let mut s = Session::new();
        s.execute("file.new", json!({"width": 160, "height": 120, "background": "#ffffff"})).unwrap();
        // Paint horizontal-ish stripes tilted by 3° straight into the background.
        s.edit("test paint", |doc, _| {
            let t = 3.0f64.to_radians().tan();
            let l = doc.layers.first_mut().unwrap();
            let surf = l.surface_mut().unwrap();
            for y in 0..120 {
                for x in 0..160 {
                    // Smooth stripes sloping down to the right by 3°.
                    let v = y as f64 - t * x as f64;
                    let g = (0.5 + 0.4 * (std::f64::consts::TAU * v / 12.0).sin()) as f32;
                    surf.write_pixel(x, y, &[g, g, g, 1.0]);
                }
            }
            Ok(())
        })
        .unwrap();
        let before = s.active().unwrap().doc.size;
        let m = s.execute("image.autoStraighten", json!({"apply": false})).unwrap();
        assert_eq!(m["applied"], false);
        assert!((m["angle"].as_f64().unwrap() + 3.0).abs() < 0.8, "{m}");
        assert_eq!(s.active().unwrap().doc.size, before, "measuring changes nothing");
        let r = s.execute("image.autoStraighten", json!({})).unwrap();
        assert_eq!(r["applied"], true, "{r}");
        let after = s.active().unwrap().doc.size;
        assert!(after.width <= before.width && after.height <= before.height, "cropped to the inscribed rectangle");
        s.undo();
        assert_eq!(s.active().unwrap().doc.size, before, "one undo step restores it");
        assert!(s.execute("image.autoStraighten", json!({"apply": "yes"})).is_err());
        assert!(s.execute("image.autoStraighten", json!({"maxAngle": "x"})).is_err());
    }

    #[test]
    fn auto_straighten_leaves_a_blank_or_level_image_alone() {
        let mut s = session(8);
        let r = s.execute("image.autoStraighten", json!({})).unwrap();
        assert_eq!(r["applied"], false);
        assert_eq!(s.active().unwrap().doc.size.width, 48);
        assert!(Session::new().execute("image.autoStraighten", json!({})).is_err(), "no document");
    }
}
