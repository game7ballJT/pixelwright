# Pixelwright app icon

A pixel-art **P** built from rounded cells on a blue gradient tile, with one amber "spark" cell
where the bowl of the P turns. It is original artwork made for Pixelwright; it replaces the
upstream project's icon, which is not used anywhere in this repository.

## Palette

| Colour | Hex | Used for |
|---|---|---|
| Ink | `#0e1233` | the tile gradient, top-left |
| Pixelwright blue | `#3b6cff` | the tile gradient, bottom-right |
| Paper | `#f4f1e8` | the P's cells |
| Spark | `#ffb02e` | the one accent cell |

## Geometry

A 512-unit tile (`viewBox="0 0 512 512"`), rounded square with `rx=112`. Cells are 64 units with a
12-unit gap and `rx=14`, on a 4 × 5 grid centred in the tile, each with a soft 7-unit shadow. The
macOS renders pad the tile onto Apple's 824/1024 grid; the Windows and Linux renders fill the canvas.

## Files

- `pixelwright.svg`, `pixelwright-small.svg`: the vector master (identical; the second name is kept
  for the packaging scripts that expect it), also the hicolor scalable icon.
- `pixelwright-1024.png`: 1024 px render on the macOS grid.
- `pixelwright.icns`: macOS bundle icon (`CFBundleIconFile`).
- `pixelwright.ico`: Windows icon, 16–256 px, embedded in the `.exe` by `apps/pixelwright/build.rs`.
- `hicolor/<size>/apps/online.jonmcgee.pixelwright.png` (16–512) and `hicolor/scalable/...svg`:
  Linux icon theme.

The app also sets the window icon and Wayland app ID at runtime (`apps/pixelwright/src/app_icon.rs`,
`main.rs`). On Windows the taskbar shows the icon of the Start Menu shortcut that launches the exe,
so the MSI's `<Icon Id>` keeps the `.exe` extension (ICE50); `app_icon.rs` documents the details
and tests the `.ico` sizes and the WiX icon references.

## Regenerate

Edit the drawing at the top of `packaging/icons.ps1` and run it from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File packaging/icons.ps1
```

It needs only Windows PowerShell (System.Drawing); no SVG renderer is required.
