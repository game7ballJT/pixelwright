# What this fork changes

Pixelwright is a fork of [PhotoCraft](https://github.com/storytold/photocraft) v0.5.0 (commit
`e5e3e39`, 2026-10-08). The upstream history is preserved in this repository, so `git log` shows who
wrote what; this page lists what the fork changed on top of it.

## Rebrand

- Every crate, binary, app ID, string, document and test is renamed: `photocraft-*` is
  `pixelwright-*`, the app ID is `online.jonmcgee.pixelwright`, the native format extension is
  `.pxwr` (files saved by upstream, `.pcraft`, still open: the bundle layout is identical and is
  detected by content).
- The upstream brand is removed: `docs/brand/` (its logos are not open source), the Discord and
  website links, the sibling-app table. The Help menu links to this repository and, as the credit
  the fork owes, to the upstream project. A new, original app icon is drawn in code
  (`packaging/icons.ps1`).
- Legal files kept and corrected: `LICENSE-MIT` names both the original copyright holders and the
  fork's, `LICENSE-APACHE` is untouched, `NOTICE` and `ATTRIBUTION.md` say what is upstream and
  what is new.

## New features

| Feature | Command | Notes |
|---|---|---|
| Frequency Separation | `layer.frequencySeparation` | Low (blurred) and high (Linear Light) layers in a group; reconstructs the original at 8, 16 and 32 bits |
| Dodge & Burn Layer | `layer.newDodgeBurn` | 50 % grey, Overlay or Soft Light |
| Orton Glow | `layer.ortonGlow` | Blurred Screen copy; opacity is the amount |
| Pixel Art | `filter.pixelate.pixelArt` | Cell average, palette reduction with dithering, nearest-neighbour up-scale; honours the selection |
| Straighten Horizon | `image.autoStraighten` | Edge-orientation voting on a smoothed grey thumbnail; confidence score; shares the Ruler Straighten's rotate-and-crop |
| Swatch library | `swatch.presets.*` | Groups, persistence, `.aco` / `.ase` import and export, palette from image, panel UI |
| Tritanopia and Achromatopsia proofing | `view.proofSetup.colorBlindness…` | Machado et al. (2009) matrix for tritanopia; luminance-only for achromatopsia |
| Last folder remembered | desktop services | File › Open, Save As start where the last dialog ended (`last-folder.txt` in the config directory) |
| Recent commands | command palette | An empty query lists what was run last, newest first |
| `.pcraft` read compatibility | format, io, engine, web | `pixelwright_format::LEGACY_EXTENSION` |
| Web `?open=` and samples | `pixelwright-web` | Same-origin paths only (`is_same_origin_path`); four public-domain samples on the start screen |
| `pixelwright-cli palette` / `diff` | CLI | See the README |

The ACO and ASE codecs live in the standalone `pixelwright-psd` crate next to `abr` and `grd`
(truncation and bit-flip sweeps in the tests). The swatch import and export commands are on the
automation sandbox's filesystem allow-list (`crates/automation/src/workspace.rs`).

## Fixes

- The shipped synthetic CMYK ICC profile carried the old product name in its description; it is
  regenerated (`PIXELWRIGHT_REGEN_PROFILES=1 cargo test -p pixelwright-cms --release --test regen`).
- The example invert plug-in and its test fixture are rebuilt with the new plug-in ID.
- A test fixture that depended on the length of the old product name (an EXIF "Make" string) no
  longer does.
- Two upstream scorecard rows were stale: *Sample All Layers applies to Blur, Sharpen and Smudge*
  and *Smudge mixes premultiplied RGBA*. Both are implemented and tested upstream; the rows are
  verified against the code and flipped to done (`scorecard/bugs.toml`, regenerated
  `docs/scorecard.md`).
- The upstream stray log files (`fb-clippy.log`, `fb-test.log`) are removed, and `SECURITY.md`
  names a real reporting channel (GitHub private vulnerability reporting).

## What the checks say

The first full run after the rename was 3,948 tests passing and 5 failing, all five caused by the
rename (the ones above plus the translation-coverage test, which correctly demanded translations of
the new menu labels). After the fixes, clippy is clean with `-D warnings` on every touched crate, the
layering check passes (`cargo xtask layers`), the new translations cover all 12 translated UI
languages, and `cargo test -p pixelwright-engine --test panic_hunt -- --ignored` fuzzes the new
commands with adversarial parameters along with the rest.

## Still open (not touched here)

Everything listed as missing or partial in [`scorecard.md`](scorecard.md) and
[`roadmap.md`](roadmap.md) that is not named above. In particular: the GPU compositor's invalid
uniform buffer in the 150-layer nudge benchmark (it needs a GPU to reproduce and was not
investigated), generative and ML features, missing tools, type on a path, SVG and PDF import, and
the 57 preferences that nothing reads.
