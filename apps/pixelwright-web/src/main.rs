//! Pixelwright in the browser.
//!
//! Runs the same [`pixelwright_ui_egui::PixelwrightApp`] as the desktop app through eframe's web
//! runner (wgpu: WebGPU where available, WebGL2 otherwise). Build with `trunk build --release`
//! from this directory; see `docs/development.md` ("Web build").
//!
//! Differences from the desktop app:
//! - no TCP control server (browsers can't listen on sockets);
//! - File → Open uses the browser file picker; bytes arrive asynchronously through
//!   `Services::inbox`;
//! - saving/exporting triggers a browser download;
//! - dropped files are read asynchronously by `web::WebShell` and delivered through the inbox.
//!
//! URL query flags: `?cpu` forces the CPU canvas path (same as `PIXELWRIGHT_CPU_CANVAS=1`);
//! `?webgl` forces the WebGL2 backend instead of WebGPU; `?open=<same-origin path>` fetches that
//! file (a bundled sample image, say) and opens it at startup.

#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

#[cfg(target_arch = "wasm32")]
mod web;

/// Whether `url` is a plain same-origin path (`samples/wave.jpg`, `/apps/x/a.png`), never another
/// site or a scheme: `?open=` is for pages that bundle their own samples, and a crafted link must
/// not make the editor fetch from anywhere else.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn is_same_origin_path(url: &str) -> bool {
    !url.is_empty() && !url.starts_with("//") && !url.contains(':') && !url.contains('\\') && !url.split('/').any(|part| part == "..")
}

#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    eprintln!("pixelwright-web only runs in the browser: build it with `trunk build --release` in apps/pixelwright-web");
}

#[cfg(test)]
mod tests {
    #[test]
    fn open_param_accepts_only_same_origin_paths() {
        for ok in ["samples/wave.jpg", "/apps/pixelwright/samples/a.png", "a.png"] {
            assert!(super::is_same_origin_path(ok), "{ok}");
        }
        for bad in [
            "",
            "https://evil.example/a.png",
            "//evil.example/a.png",
            "javascript:alert(1)",
            "data:image/png;base64,AA",
            "../secret.png",
            "a/../../b.png",
            "a\\b.png",
        ] {
            assert!(!super::is_same_origin_path(bad), "{bad}");
        }
    }

    /// A WebAssembly module that never downloads (or doesn't match the page) runs no Rust, so the
    /// page itself replaces the endless "Loading" with what may have gone wrong and a way to
    /// retry, without a hand-written script.
    #[test]
    fn loading_page_offers_a_reload_when_startup_stalls() {
        let html = include_str!("../index.html");
        let (_, hint) = html.split_once("<p id=\"pixelwright_stalled\">").expect("stalled-startup hint in the loading element");
        assert!(hint.contains("failed to") && hint.contains("<a href=\"\">Reload</a>"), "{hint}");
        assert!(html.contains("animation: pixelwright-stalled"), "the hint appears after a delay");
        assert!(!html.contains("<script"), "trunk injects the generated loader; the page has no script of its own");
    }
}
