# TOL brand

Short version: black, one red, one eye, and the letters `TOL`. Everything else
is optional.

## The mark

Two elements, used together on banners and apart where space is short:

1. **The eye** — a rectangle split into two halves, black and red, with a lens
   between them. It is a drawing of the thing this project cares about:
   looking at the disk before touching it.
2. **The wordmark** — `TOL` in a heavy grotesque, white on black.

The full composition is the eye on the left, the wordmark on the right. Do not
rearrange it into a vertical lockup unless you are making a square avatar, and
then use the eye alone.

## Attribution — required

The eye follows the graphic language of the band **TOOL**. That is inspiration,
not a partnership, and the difference has to be written down wherever the mark
is published:

> The TOL mark is inspired by the visual language of the band TOOL. TOL is not
> affiliated with, endorsed by, or connected to TOOL, its members or its
> management.

Keep that line in the organisation profile, the repository README and any page
that shows the banner. If TOL ever sells something — hardware, support, a
paid build — replace the mark first. A lookalike of a band's identity is a bad
asset to build a business on, even when the band is friendly about it.

## Colours

| Token | Value | Use |
|---|---|---|
| `ink` | `#000000` | background, eye's dark half |
| `paper` | `#F2F0EB` | body text on dark, light backgrounds |
| `signal` | sample from the banner (`assets/tol/banner.png`) | the eye's red half, accents |
| `dim` | `#6E6A63` | secondary text, rules |

`signal` is deliberately not guessed here: the red in the artwork is the source
of truth. Sample it once from the PNG, write the hex into this table, and stop
inventing new reds. Until then, do not place the mark next to a different red.

Only these four. No gradients, no glow, no drop shadows. The mark works because
it is flat.

## Typography

- Wordmark: heavy grotesque, tight tracking, uppercase. The artwork is the
  reference; do not retype `TOL` in a similar font and call it the logo.
- Body: whatever the surrounding document uses. TOL does not ship a font.

## Files

Place the source art here:

| File | What it is |
|---|---|
| `assets/tol/banner.png` | wide banner, eye + wordmark, for README headers and the organisation profile |
| `assets/tol/logo.png` | square or near-square mark, for avatars |
| `assets/tol/logo-transparent.png` | optional: mark with the background removed |

Requirements for the two required files:

- PNG, at least 2× the display size (banner ≥ 1600 px wide, avatar ≥ 512 px).
- No JPEG artefacts around the red/black edge.
- Keep the black background in `banner.png`. It is the design, not a leftover.

**About "removing the background":** a black-and-red mark on a black field
cannot be cut out cleanly by a background-removal tool — the dark half of the
eye is the same colour as the field, so it disappears. What works is either

1. keep the black field and use the banner as-is (recommended), or
2. redraw the eye as SVG with transparent background, which is a new asset, not
   a cut-out, or
3. cut only the outer field and keep a thin black keyline around the mark, so
   the shape survives on light backgrounds.

Option 1 is what this repository uses. Option 2 is worth doing before the mark
appears on a light page.

## Clear space and minimum size

- Clear space: at least the height of the eye's lens on every side.
- Minimum width: 96 px for the full lockup, 32 px for the eye alone.
- Below 32 px, the lens closes up. Use the eye without the wordmark, or nothing.

## Don't

- Don't recolour the eye. It is black and one red.
- Don't stretch, rotate, outline or add a glow.
- Don't put the wordmark in a different red, or in gradient.
- Don't use the mark to imply that a fork, a package or a person is official.
- Don't print the attribution line in a size nobody can read.

## Name usage

- The group is **The Ominous Linux**, short **TOL**. Not "T.O.L." in body text.
- The installer is **camareLOL**, one word, lowercase `c`, capital `LOL`.
- The kernel line is **LOL kernel**; package names are `linux-lol`,
  `linux-lol-headers`.
- Do not write "LOL" on its own as a product name. In a terminal, outside this
  project, it means something else entirely.

## License of the assets

The mark and banner are © the TOL authors, released under
[CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/) for reuse that
keeps the attribution line above. The name "The Ominous Linux" and the `TOL`
wordmark are not licensed for endorsement of third-party products.
