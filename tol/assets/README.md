# TOL assets

Drop the source artwork here. These files are referenced by
[`../BRAND.md`](../BRAND.md) and by the repository README header.

| File | Required | Notes |
|---|---|---|
| `banner.png` | yes | wide lockup (eye left, `TOL` right), keep the black field |
| `logo.png` | yes | square or near-square eye, for GitHub avatars |
| `logo-transparent.png` | optional | only if the mark was redrawn as SVG/PNG with alpha; do not fake a cut-out of a black-on-black mark |
| `banner.svg`, `logo.svg` | optional | preferred for print and for light backgrounds |

## What "good enough" means

- Banner at least 1600 px wide; avatar at least 512 × 512.
- PNG, no JPEG artefacts on the red/black edge.
- The black field stays. A split black/red eye cannot be background-removed
  cleanly, because its dark half is the same colour as the field. See
  [`../BRAND.md`](../BRAND.md#files).
- The red is the source of truth for the `signal` colour token. Sample it and
  write the hex into the brand table.

## Until the files are here

Nothing links to a missing image. The READMEs mention TOL in text only, so a
clone without the artwork still renders. Add the images first, then point the
README at them in a separate commit — that keeps a broken image out of `main`.
