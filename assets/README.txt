Paint Rust artwork
==================

Version: 1 (2026-10-24)
Author: Paint Rust contributors, created with OpenAI Codex.
Source: the editable SVG files in this directory and icons/.
License: MIT OR Apache-2.0; see the root LICENSE-MIT and LICENSE-APACHE files.

All application artwork here was created for this project. No external icon pack
or reference artwork was used to trace these shapes. Conventional symbols express
common actions; the SVG geometry is maintained in this repository.

The 22 action/tool icons use a 24x24 viewBox, 1.7-unit rounded strokes and white
geometry. The UI tints them with the current text color, including selected and
disabled states. Labels and shortcut hints remain visible beside the icons.
The app icon uses a separate color design and is embedded in the native window.
Regenerate its PNG with:

    magick -background none assets/app-icon.svg assets/app-icon.png

Future artwork must be created for this project or supplied under MIT or
Apache-2.0. For external artwork, record the upstream URL, author, exact version,
license, modifications and required notices before adding it to the interface.
