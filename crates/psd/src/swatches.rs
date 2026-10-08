//! Adobe swatch files: Color Swatch (`.aco`, versions 1 and 2) and Swatch Exchange (`.ase`).
//!
//! Implemented from the public descriptions of both formats.
//!
//! * **ACO**: big-endian. A version-1 section (`u16 1`, `u16 count`, then per colour a `u16`
//!   colour space and four `u16` components) usually followed by a version-2 section (`u16 2`,
//!   `u16 count`, the same per-colour data plus a `u32` name length in UTF-16 units including the
//!   terminating NUL, and the UTF-16BE name). Colour spaces: 0 RGB (0..65535), 1 HSB (hue
//!   0..65535 for 0..360°, the others 0..65535), 2 CMYK (0 is *full* ink), 7 Lab (L 0..10000 for
//!   0..100, a and b signed ×100), 8 Gray (0..10000, 10000 = white). Spot-colour libraries
//!   (Pantone and the like) are not decoded and are counted as skipped.
//! * **ASE**: `ASEF`, version `u16 1`, `u16 0`, `u32` block count; blocks are `u16` type,
//!   `u32` length and a payload: `0xC001` group start and `0xC002` group end (a name in UTF-16BE
//!   with a `u16` length that includes the NUL), `0x0001` colour entry (name, a four-character
//!   model `RGB `, `CMYK`, `LAB ` or `Gray`, big-endian `f32` components, a `u16` colour type).
//!
//! Colours come back in the model they were stored in; converting them is the caller's job.
//! Parsers never panic on malformed input and bound every count.

use crate::error::{PsdError, Result};
use crate::io::Reader;

/// Most swatches read from one file.
pub const MAX_SWATCHES: usize = 100_000;
/// Longest name read, in UTF-16 units.
const MAX_NAME_UNITS: usize = 4_096;

/// A swatch colour as stored.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SwatchColor {
    /// 0..1 RGB.
    Rgb([f32; 3]),
    /// Hue in degrees (0..360), saturation and brightness 0..1.
    Hsb([f32; 3]),
    /// 0..1 ink.
    Cmyk([f32; 4]),
    /// L 0..100, a, b.
    Lab([f32; 3]),
    /// 0..1 lightness (1 = white).
    Gray(f32),
}

/// One swatch.
#[derive(Debug, Clone, PartialEq)]
pub struct Swatch {
    /// Display name (may be empty: ACO version 1 has none).
    pub name: String,
    /// The colour.
    pub color: SwatchColor,
}

/// A named run of swatches. ACO files have one unnamed group; ASE files may have several.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SwatchGroup {
    /// Group name (empty for colours outside any ASE group).
    pub name: String,
    /// The group's swatches, in file order.
    pub swatches: Vec<Swatch>,
}

/// What a file held: the groups, and how many entries could not be decoded (spot-colour
/// libraries, unknown colour models).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SwatchFile {
    /// Decoded groups (never empty groups; at least one when anything decoded).
    pub groups: Vec<SwatchGroup>,
    /// Entries skipped because their colour model is not supported.
    pub skipped: usize,
}

fn unit(v: u16) -> f32 {
    f32::from(v) / 65535.0
}

fn from_aco(space: u16, c: [u16; 4]) -> Option<SwatchColor> {
    Some(match space {
        0 => SwatchColor::Rgb([unit(c[0]), unit(c[1]), unit(c[2])]),
        1 => SwatchColor::Hsb([unit(c[0]) * 360.0, unit(c[1]), unit(c[2])]),
        2 => SwatchColor::Cmyk([1.0 - unit(c[0]), 1.0 - unit(c[1]), 1.0 - unit(c[2]), 1.0 - unit(c[3])]),
        7 => SwatchColor::Lab([f32::from(c[0]) / 100.0, f32::from(c[1] as i16) / 100.0, f32::from(c[2] as i16) / 100.0]),
        8 => SwatchColor::Gray((f32::from(c[0]) / 10_000.0).clamp(0.0, 1.0)),
        _ => return None,
    })
}

fn to_aco(c: &SwatchColor) -> (u16, [u16; 4]) {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 65535.0).round() as u16;
    match *c {
        SwatchColor::Rgb(v) => (0, [q(v[0]), q(v[1]), q(v[2]), 0]),
        SwatchColor::Hsb(v) => (1, [q(v[0] / 360.0), q(v[1]), q(v[2]), 0]),
        SwatchColor::Cmyk(v) => (2, [65535 - q(v[0]), 65535 - q(v[1]), 65535 - q(v[2]), 65535 - q(v[3])]),
        SwatchColor::Lab(v) => (
            7,
            [
                (v[0].clamp(0.0, 100.0) * 100.0).round() as u16,
                (v[1].clamp(-128.0, 127.0) * 100.0).round() as i16 as u16,
                (v[2].clamp(-128.0, 127.0) * 100.0).round() as i16 as u16,
                0,
            ],
        ),
        SwatchColor::Gray(v) => (8, [(v.clamp(0.0, 1.0) * 10_000.0).round() as u16, 0, 0, 0]),
    }
}

fn units_to_string(units: &[u16]) -> String {
    let end = units.iter().position(|u| *u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(units.get(..end).unwrap_or(&[]))
}

/// A UTF-16BE string of `n` units (the count includes the NUL).
fn read_name(r: &mut Reader<'_>, n: usize) -> Result<String> {
    if n > MAX_NAME_UNITS {
        return Err(PsdError::LimitExceeded("swatch name too long"));
    }
    let bytes = r.bytes(n * 2)?;
    let units: Vec<u16> = bytes.as_chunks::<2>().0.iter().map(|p| u16::from_be_bytes(*p)).collect();
    Ok(units_to_string(&units))
}

fn write_name(out: &mut Vec<u8>, name: &str, wide_len: bool) {
    let units: Vec<u16> = name.encode_utf16().take(MAX_NAME_UNITS - 1).collect();
    let n = units.len() + 1;
    if wide_len {
        out.extend_from_slice(&(n as u32).to_be_bytes());
    } else {
        out.extend_from_slice(&(n as u16).to_be_bytes());
    }
    for u in units {
        out.extend_from_slice(&u.to_be_bytes());
    }
    out.extend_from_slice(&0u16.to_be_bytes());
}

// ---------------------------------------------------------------------------------------------
// ACO

/// Parse a `.aco` file (version 1, or version 2 with names; the version-2 section wins when
/// present). A single group is returned.
pub fn parse_aco(data: &[u8]) -> Result<SwatchFile> {
    let mut r = Reader::new(data);
    let first = r.u16()?;
    if first != 1 && first != 2 {
        return Err(PsdError::invalid(format!("not a Color Swatch file (version {first})")));
    }
    let section = |r: &mut Reader<'_>, named: bool| -> Result<(Vec<Swatch>, usize)> {
        let count = usize::from(r.u16()?);
        r.check_count(count as u64, 10)?;
        let (mut out, mut skipped) = (Vec::with_capacity(count), 0usize);
        for _ in 0..count.min(MAX_SWATCHES) {
            let space = r.u16()?;
            let c = [r.u16()?, r.u16()?, r.u16()?, r.u16()?];
            let name = if named {
                let len = r.u32()? as usize;
                read_name(r, len)?
            } else {
                String::new()
            };
            match from_aco(space, c) {
                Some(color) => out.push(Swatch { name, color }),
                None => skipped += 1,
            }
        }
        Ok((out, skipped))
    };
    let (swatches, skipped) = if first == 1 {
        let v1 = section(&mut r, false)?;
        // A version-2 section (with names) normally follows; a damaged one keeps version 1.
        let v2 = if r.remaining() >= 4 && r.u16().is_ok_and(|v| v == 2) { section(&mut r, true).ok().filter(|(s, sk)| !s.is_empty() || *sk > 0) } else { None };
        v2.unwrap_or(v1)
    } else {
        section(&mut r, true)?
    };
    let groups = if swatches.is_empty() { Vec::new() } else { vec![SwatchGroup { name: String::new(), swatches }] };
    Ok(SwatchFile { groups, skipped })
}

/// Write a `.aco` file with a version-1 and a version-2 (named) section, so every reader opens it.
/// At most 65 535 swatches are written (the format's count field).
pub fn write_aco(swatches: &[Swatch]) -> Vec<u8> {
    let list: Vec<&Swatch> = swatches.iter().take(usize::from(u16::MAX)).collect();
    let mut out = Vec::new();
    for version in [1u16, 2] {
        out.extend_from_slice(&version.to_be_bytes());
        out.extend_from_slice(&(list.len() as u16).to_be_bytes());
        for s in &list {
            let (space, c) = to_aco(&s.color);
            out.extend_from_slice(&space.to_be_bytes());
            for v in c {
                out.extend_from_slice(&v.to_be_bytes());
            }
            if version == 2 {
                write_name(&mut out, &s.name, true);
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// ASE

const ASE_GROUP_START: u16 = 0xC001;
const ASE_GROUP_END: u16 = 0xC002;
const ASE_COLOR: u16 = 0x0001;

fn f32_at(r: &mut Reader<'_>) -> Result<f32> {
    Ok(f32::from_bits(r.u32()?))
}

/// Parse a `.ase` file. Colours outside any group land in an unnamed group; spot and global
/// colour types are read as ordinary colours.
pub fn parse_ase(data: &[u8]) -> Result<SwatchFile> {
    let mut r = Reader::new(data);
    let sig = r.array::<4>()?;
    if &sig != b"ASEF" {
        return Err(PsdError::InvalidSignature { expected: "ASEF", found: sig });
    }
    let (major, _minor) = (r.u16()?, r.u16()?);
    if major != 1 {
        return Err(PsdError::UnsupportedVersion(major));
    }
    let blocks = r.u32()? as usize;
    r.check_count(blocks as u64, 6)?;
    let mut file = SwatchFile::default();
    let mut current = SwatchGroup::default();
    let mut total = 0usize;
    for _ in 0..blocks.min(MAX_SWATCHES * 2) {
        let ty = r.u16()?;
        let len = r.u32()? as u64;
        let mut body = r.sub(len)?;
        match ty {
            ASE_GROUP_START => {
                if !current.swatches.is_empty() {
                    file.groups.push(std::mem::take(&mut current));
                }
                let n = usize::from(body.u16()?);
                current = SwatchGroup { name: read_name(&mut body, n)?, swatches: Vec::new() };
            }
            ASE_GROUP_END => {
                if !current.swatches.is_empty() {
                    file.groups.push(std::mem::take(&mut current));
                }
                current = SwatchGroup::default();
            }
            ASE_COLOR => {
                total += 1;
                if total > MAX_SWATCHES {
                    return Err(PsdError::LimitExceeded("too many swatches"));
                }
                let n = usize::from(body.u16()?);
                let name = read_name(&mut body, n)?;
                let model = body.array::<4>()?;
                let color = match &model {
                    b"RGB " => Some(SwatchColor::Rgb([f32_at(&mut body)?, f32_at(&mut body)?, f32_at(&mut body)?])),
                    b"CMYK" => Some(SwatchColor::Cmyk([f32_at(&mut body)?, f32_at(&mut body)?, f32_at(&mut body)?, f32_at(&mut body)?])),
                    b"LAB " => {
                        let l = f32_at(&mut body)?;
                        Some(SwatchColor::Lab([l * 100.0, f32_at(&mut body)?, f32_at(&mut body)?]))
                    }
                    b"Gray" => Some(SwatchColor::Gray(f32_at(&mut body)?)),
                    _ => None,
                };
                match color {
                    Some(color) if color_is_finite(&color) => current.swatches.push(Swatch { name, color }),
                    _ => file.skipped += 1,
                }
            }
            _ => {}
        }
    }
    if !current.swatches.is_empty() {
        file.groups.push(current);
    }
    Ok(file)
}

fn color_is_finite(c: &SwatchColor) -> bool {
    match c {
        SwatchColor::Rgb(v) | SwatchColor::Hsb(v) | SwatchColor::Lab(v) => v.iter().all(|x| x.is_finite()),
        SwatchColor::Cmyk(v) => v.iter().all(|x| x.is_finite()),
        SwatchColor::Gray(v) => v.is_finite(),
    }
}

fn ase_block(out: &mut Vec<u8>, ty: u16, body: &[u8]) {
    out.extend_from_slice(&ty.to_be_bytes());
    out.extend_from_slice(&(body.len() as u32).to_be_bytes());
    out.extend_from_slice(body);
}

/// Write a `.ase` file. Each non-empty group name becomes a group block; swatches in a group
/// with an empty name are written outside any group.
pub fn write_ase(groups: &[SwatchGroup]) -> Vec<u8> {
    let mut blocks: Vec<(u16, Vec<u8>)> = Vec::new();
    let name_bytes = |name: &str| {
        let mut b = Vec::new();
        write_name(&mut b, name, false);
        b
    };
    for g in groups {
        if !g.name.is_empty() {
            blocks.push((ASE_GROUP_START, name_bytes(&g.name)));
        }
        for s in &g.swatches {
            let mut b = name_bytes(&s.name);
            let (model, comps): (&[u8; 4], Vec<f32>) = match s.color {
                SwatchColor::Rgb(v) => (b"RGB ", v.to_vec()),
                SwatchColor::Cmyk(v) => (b"CMYK", v.to_vec()),
                SwatchColor::Lab(v) => (b"LAB ", vec![v[0] / 100.0, v[1], v[2]]),
                SwatchColor::Gray(v) => (b"Gray", vec![v]),
                // HSB has no ASE model: store it as the RGB it stands for.
                SwatchColor::Hsb(v) => (b"RGB ", hsb_to_rgb(v).to_vec()),
            };
            b.extend_from_slice(model);
            for c in comps {
                b.extend_from_slice(&c.to_bits().to_be_bytes());
            }
            b.extend_from_slice(&2u16.to_be_bytes()); // normal colour
            blocks.push((ASE_COLOR, b));
        }
        if !g.name.is_empty() {
            blocks.push((ASE_GROUP_END, Vec::new()));
        }
    }
    let mut out = b"ASEF".to_vec();
    out.extend_from_slice(&1u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&(blocks.len() as u32).to_be_bytes());
    for (ty, body) in &blocks {
        ase_block(&mut out, *ty, body);
    }
    out
}

/// Hue in degrees, saturation and brightness 0..1 to 0..1 RGB (the standard hexcone formula).
pub fn hsb_to_rgb(v: [f32; 3]) -> [f32; 3] {
    let (h, s, b) = (v[0].rem_euclid(360.0) / 60.0, v[1].clamp(0.0, 1.0), v[2].clamp(0.0, 1.0));
    let (i, f) = (h.floor(), h - h.floor());
    let (p, q, t) = (b * (1.0 - s), b * (1.0 - s * f), b * (1.0 - s * (1.0 - f)));
    match i as i32 {
        0 => [b, t, p],
        1 => [q, b, p],
        2 => [p, b, t],
        3 => [p, q, b],
        4 => [t, p, b],
        _ => [b, p, q],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<Swatch> {
        vec![
            Swatch { name: "Brand blue".into(), color: SwatchColor::Rgb([0.0, 0.4, 1.0]) },
            Swatch { name: "Ink".into(), color: SwatchColor::Cmyk([0.1, 0.2, 0.3, 0.9]) },
            Swatch { name: "Mid grey".into(), color: SwatchColor::Gray(0.5) },
            Swatch { name: "Lab teal".into(), color: SwatchColor::Lab([55.0, -40.0, -10.0]) },
            Swatch { name: "Café ☕".into(), color: SwatchColor::Rgb([0.5, 0.25, 0.1]) },
        ]
    }

    fn close(a: SwatchColor, b: SwatchColor) -> bool {
        let v = |c: SwatchColor| -> Vec<f32> {
            match c {
                SwatchColor::Rgb(x) | SwatchColor::Hsb(x) | SwatchColor::Lab(x) => x.to_vec(),
                SwatchColor::Cmyk(x) => x.to_vec(),
                SwatchColor::Gray(x) => vec![x],
            }
        };
        std::mem::discriminant(&a) == std::mem::discriminant(&b) && v(a).iter().zip(v(b)).all(|(x, y)| (x - y).abs() < 0.02)
    }

    #[test]
    fn aco_round_trips_with_names_and_colour_models() {
        let list = sample();
        let bytes = write_aco(&list);
        let f = parse_aco(&bytes).unwrap();
        assert_eq!(f.skipped, 0);
        assert_eq!(f.groups.len(), 1);
        let got = &f.groups[0].swatches;
        assert_eq!(got.len(), list.len());
        for (a, b) in got.iter().zip(&list) {
            assert_eq!(a.name, b.name);
            assert!(close(a.color, b.color), "{:?} vs {:?}", a.color, b.color);
        }
    }

    #[test]
    fn aco_version_one_has_no_names_and_spot_colours_are_skipped() {
        // v1 only: RGB red, a Pantone-style space (3) and gray.
        let mut b = Vec::new();
        b.extend_from_slice(&1u16.to_be_bytes());
        b.extend_from_slice(&3u16.to_be_bytes());
        for (space, c) in [(0u16, [65535u16, 0, 0, 0]), (3, [1, 2, 3, 4]), (8, [10000, 0, 0, 0])] {
            b.extend_from_slice(&space.to_be_bytes());
            for v in c {
                b.extend_from_slice(&v.to_be_bytes());
            }
        }
        let f = parse_aco(&b).unwrap();
        assert_eq!(f.skipped, 1);
        assert_eq!(f.groups[0].swatches.len(), 2);
        assert_eq!(f.groups[0].swatches[0].name, "");
        assert!(close(f.groups[0].swatches[1].color, SwatchColor::Gray(1.0)));
    }

    #[test]
    fn ase_round_trips_groups_and_models() {
        let groups =
            vec![SwatchGroup { name: String::new(), swatches: sample()[..1].to_vec() }, SwatchGroup { name: "Brand".into(), swatches: sample()[1..].to_vec() }];
        let bytes = write_ase(&groups);
        assert_eq!(&bytes[..4], b"ASEF");
        let f = parse_ase(&bytes).unwrap();
        assert_eq!(f.groups.len(), 2);
        assert_eq!(f.groups[0].name, "");
        assert_eq!(f.groups[1].name, "Brand");
        for (g, want) in f.groups.iter().zip(&groups) {
            assert_eq!(g.swatches.len(), want.swatches.len());
            for (a, b) in g.swatches.iter().zip(&want.swatches) {
                assert_eq!(a.name, b.name);
                assert!(close(a.color, b.color), "{:?} vs {:?}", a.color, b.color);
            }
        }
    }

    #[test]
    fn hsb_is_stored_as_the_rgb_it_stands_for_in_ase() {
        let g = vec![SwatchGroup { name: String::new(), swatches: vec![Swatch { name: "red".into(), color: SwatchColor::Hsb([0.0, 1.0, 1.0]) }] }];
        let f = parse_ase(&write_ase(&g)).unwrap();
        assert!(close(f.groups[0].swatches[0].color, SwatchColor::Rgb([1.0, 0.0, 0.0])));
        assert_eq!(hsb_to_rgb([120.0, 1.0, 1.0]), [0.0, 1.0, 0.0]);
    }

    #[test]
    fn malformed_input_is_an_error_never_a_panic() {
        for bytes in [&[][..], &[0u8][..], b"nope", b"ASEF", &[0, 1, 255, 255][..], &[0, 2, 255, 255, 0, 0][..]] {
            let _ = parse_aco(bytes);
            let _ = parse_ase(bytes);
        }
        assert!(parse_aco(b"\x00\x09\x00\x00").is_err(), "unknown version");
        assert!(matches!(parse_ase(b"XXXX\x00\x01\x00\x00\x00\x00\x00\x00"), Err(PsdError::InvalidSignature { .. })));
        assert!(matches!(parse_ase(b"ASEF\x00\x02\x00\x00\x00\x00\x00\x00"), Err(PsdError::UnsupportedVersion(2))));
        // A huge block count cannot allocate: it fails when the data runs out.
        assert!(parse_ase(b"ASEF\x00\x01\x00\x00\xff\xff\xff\xff").is_err());
        assert!(parse_aco(b"\x00\x01\xff\xff").is_err());
    }

    #[test]
    fn truncating_a_valid_file_at_every_offset_never_panics() {
        let aco = write_aco(&sample());
        let ase = write_ase(&[SwatchGroup { name: "G".into(), swatches: sample() }]);
        for n in 0..aco.len() {
            let _ = parse_aco(&aco[..n]);
        }
        for n in 0..ase.len() {
            let _ = parse_ase(&ase[..n]);
        }
        // Flip every byte of a small file in turn.
        for i in 0..ase.len() {
            let mut b = ase.clone();
            b[i] ^= 0xff;
            let _ = parse_ase(&b);
        }
    }

    #[test]
    fn non_finite_ase_components_are_skipped_not_trusted() {
        let mut body = Vec::new();
        write_name(&mut body, "bad", false);
        body.extend_from_slice(b"RGB ");
        for v in [f32::NAN, 0.0, 1.0] {
            body.extend_from_slice(&v.to_bits().to_be_bytes());
        }
        body.extend_from_slice(&2u16.to_be_bytes());
        let mut file = b"ASEF\x00\x01\x00\x00\x00\x00\x00\x01".to_vec();
        ase_block(&mut file, ASE_COLOR, &body);
        let f = parse_ase(&file).unwrap();
        assert_eq!((f.groups.len(), f.skipped), (0, 1));
    }
}
