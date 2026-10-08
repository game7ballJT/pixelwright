# Release playbook

The upstream project (PhotoCraft) keeps a shared release playbook in a private repository of its authors.
Pixelwright follows its shape, simplified for a fork with no signing secrets; the live recipe is
the workflow below.

Pixelwright's release flow lives in `.github/workflows/release.yml`, `packaging/` and
`cargo xtask version`. Pixelwright-specific details are in [releasing.md](releasing.md).
