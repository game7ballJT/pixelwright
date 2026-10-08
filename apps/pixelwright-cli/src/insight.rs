//! Two read-only inspection subcommands: `palette` (the dominant colours of any image or
//! document) and `diff` (how far apart two images are, with an optional heat-map).
//!
//! `palette` runs the engine command `swatch.presets.fromImage`, so it matches the Swatches panel.
//! `diff` compares the flattened composites of the two files, whatever their formats.

use std::io::Write;
use std::path::Path;

use pixelwright_automation::{Headless, files};
use serde_json::{Value, json};

use crate::{Args, R, print_json};

/// `palette <file> [--count N] [--sort prominence|hue|lightness] [--format text|json|hex]`
pub(crate) fn palette(a: &Args, out: &mut dyn Write, _err: &mut dyn Write) -> R {
    let [file] = a.positional.as_slice() else {
        return Err("palette needs <file>".into());
    };
    let count: usize = match a.get("--count") {
        None => 8,
        Some(c) => c.parse().ok().filter(|n| (2..=64).contains(n)).ok_or_else(|| format!("bad --count `{c}`: expected a whole number from 2 to 64"))?,
    };
    let sort = a.get("--sort").unwrap_or("prominence");
    if !["prominence", "hue", "lightness"].contains(&sort) {
        return Err(format!("bad --sort `{sort}`: expected prominence, hue or lightness"));
    }
    let format = a.get("--format").unwrap_or("text");
    if !["text", "json", "hex"].contains(&format) {
        return Err(format!("bad --format `{format}`: expected text, json or hex"));
    }
    let mut h = Headless::trusted_local();
    h.open(Path::new(file)).map_err(|e| format!("{file}: {e}"))?;
    let result = h.command_run("swatch.presets.fromImage", json!({"count": count, "sort": sort})).map_err(|e| e.to_string())?;
    let colors = result["colors"].as_array().cloned().unwrap_or_default();
    match format {
        "json" => print_json(out, &Value::Array(colors), false),
        "hex" => {
            for c in &colors {
                writeln!(out, "{}", c["color"].as_str().unwrap_or_default()).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
        _ => {
            for c in &colors {
                let share = c["share"].as_f64().unwrap_or(0.0);
                let bar = "█".repeat((share * 40.0).round().clamp(0.0, 40.0) as usize);
                writeln!(out, "{}  {:>5.1}%  {bar}", c["color"].as_str().unwrap_or_default(), share * 100.0).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }
}

/// The flattened composite of `path` as (width, height, straight RGBA8).
fn render(path: &str) -> Result<(u32, u32, Vec<u8>), String> {
    let opened = files::open(Path::new(path)).map_err(|e| format!("{path}: {e}"))?;
    let side = opened.document.size.width.max(opened.document.size.height).max(1);
    let img = pixelwright_compose::thumbnail(&opened.document, side);
    Ok((img.width, img.height, img.pixels))
}

fn round(v: f64, places: i32) -> f64 {
    let k = 10f64.powi(places);
    (v * k).round() / k
}

/// `diff <a> <b> [--threshold N] [--out heatmap.png] [--fail]`
///
/// Prints JSON: size, mean absolute error (0..255), RMSE, PSNR in dB (`null` when identical),
/// the largest channel difference, and how many pixels (and what percentage) differ by more than
/// `--threshold` (default 0). `--out` writes a heat-map (black = equal, red = different, ×4).
/// `--fail` exits 1 when any pixel exceeds the threshold, so scripts and CI can gate on it.
pub(crate) fn diff(a: &Args, out: &mut dyn Write, _err: &mut dyn Write) -> R {
    let [x, y] = a.positional.as_slice() else {
        return Err("diff needs <a> <b>".into());
    };
    let threshold: u8 = match a.get("--threshold") {
        None => 0,
        Some(t) => t.parse().map_err(|_| format!("bad --threshold `{t}`: expected a whole number from 0 to 255"))?,
    };
    let (wa, ha, pa) = render(x)?;
    let (wb, hb, pb) = render(y)?;
    if (wa, ha) != (wb, hb) {
        print_json(out, &json!({"sameSize": false, "a": [wa, ha], "b": [wb, hb]}), false)?;
        return Err(format!("the images differ in size: {wa}x{ha} against {wb}x{hb}"));
    }
    let pixels = u64::from(wa) * u64::from(ha);
    let (mut sum_abs, mut sum_sq, mut max_delta, mut differing) = (0u64, 0u64, 0u8, 0u64);
    let mut heat: Option<Vec<u8>> = a.get("--out").map(|_| Vec::with_capacity(pa.len()));
    for (p, q) in pa.as_chunks::<4>().0.iter().zip(pb.as_chunks::<4>().0.iter()) {
        let mut worst = 0u8;
        for d in p.iter().zip(q).map(|(c, d)| c.abs_diff(*d)) {
            sum_abs += u64::from(d);
            sum_sq += u64::from(d) * u64::from(d);
            worst = worst.max(d);
        }
        max_delta = max_delta.max(worst);
        differing += u64::from(worst > threshold);
        if let Some(h) = heat.as_mut() {
            h.extend_from_slice(&[(u16::from(worst) * 4).min(255) as u8, 0, 0, 255]);
        }
    }
    let samples = (pixels * 4).max(1) as f64;
    let (mae, rmse) = (sum_abs as f64 / samples, (sum_sq as f64 / samples).sqrt());
    let psnr = (rmse > 0.0).then(|| round(20.0 * (255.0 / rmse).log10(), 2));
    if let (Some(path), Some(h)) = (a.get("--out"), heat) {
        let img = pixelwright_codecs::Image::from_u8(wa, ha, pixelwright_codecs::ChannelLayout::Rgba, h).map_err(|e| e.to_string())?;
        let png = pixelwright_codecs::encode(&img, pixelwright_codecs::Format::Png, &Default::default()).map_err(|e| e.to_string())?;
        pixelwright_format::atomic_write(Path::new(path), &png).map_err(|e| format!("{path}: {e}"))?;
    }
    let percent = if pixels == 0 { 0.0 } else { differing as f64 * 100.0 / pixels as f64 };
    print_json(
        out,
        &json!({
            "sameSize": true, "width": wa, "height": ha,
            "meanAbsError": round(mae, 3), "rmse": round(rmse, 3), "psnrDb": psnr,
            "maxDelta": max_delta, "threshold": threshold,
            "differingPixels": differing, "percentDifferent": round(percent, 3),
        }),
        false,
    )?;
    if a.has("--fail") && differing > 0 {
        return Err(format!("{differing} pixels differ by more than {threshold}"));
    }
    Ok(())
}
