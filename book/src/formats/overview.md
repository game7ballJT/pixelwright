# Format overview

External bytes enter Pixelwright through three main paths:

- `pixelwright-psd` parses and writes PSD and PSB structures.
- `pixelwright-codecs` decodes and encodes flat raster formats.
- `pixelwright-format` loads and saves the native `.pxwr` bundle.

`pixelwright-io` maps PSD and flat images into the document model and exports documents back to those formats. It is the integration layer; parsing limits remain the responsibility of the underlying format crates.

Formats are a primary security boundary. File extension is not a trust signal, metadata can be attacker-controlled, and small compressed inputs can declare very large decoded outputs. Callers should use the bounded default APIs and preserve errors rather than retrying with unlimited limits.

See [PSD and PSB](psd-psb.md), [Raster formats](raster-formats.md), and [The `.pxwr` format](pxwr.md). Cross-format hardening guidance is in [Parser hardening](../security/parser-hardening.md).
