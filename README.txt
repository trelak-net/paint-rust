Paint Rust
==========

A native desktop raster editor written in Rust. Version 0.1 provides a small,
usable editing workspace with an RGBA canvas and transaction-based undo/redo.

Getting started
---------------
Install the stable Rust toolchain from https://rustup.rs, then run:

    cargo run --release

Open a file at startup:

    cargo run --release -- /path/to/image.png

The Linux desktop needs an OpenGL driver and either Wayland or X11. Native file
pickers use the desktop's XDG portal service. A working C linker is required to
build. Dependencies and their exact resolved versions are recorded in Cargo.lock.
The GUI uses eframe/egui 0.33.3 with the OpenGL renderer:
https://docs.rs/eframe/0.33.3/eframe/

Features
--------
- Create an opaque or transparent canvas with custom dimensions.
- Paint continuous round brush strokes; erase to transparency.
- Fill a contiguous area of exactly matching RGBA pixels.
- Pick a color from the canvas, edit RGBA values, or use palette swatches.
- Invert RGB while preserving alpha.
- Preview brightness/contrast, saturation, grayscale, sepia and alpha-aware box blur.
- Draw lines and outlined/filled rectangles or ellipses with cancellable previews.
- Select a rectangular area, limit edits to it, delete pixels or crop all layers.
- Resize all layers, change canvas size with a nine-position anchor, rotate or mirror.
- Hold Shift for 45-degree line angles or equal shape dimensions.
- Zoom from 10% to 400%, fit the canvas, and navigate with scrollbars.
- Undo/redo complete strokes and edits; cancel a stroke with Escape.
- Open PNG, JPEG, BMP and WebP; export a flattened RGBA PNG.
- Add, duplicate, reorder, hide and delete layers; edit names and opacity.
- Copy/cut active-layer pixels or copy the merged image; paste images as new layers.
- Place multiline text with a live preview, adjustable size/color and explicit Apply.
- Save editable layers in a versioned .prust project.
- Confirm unsaved changes before opening, creating or closing a document.
- Write through a temporary sibling file before replacing the destination.

Keyboard
--------
Ctrl+N: new image             Ctrl+O: open image
Ctrl+S: save project          Ctrl+Shift+S: save project as
Ctrl+Z: undo                  Ctrl+Shift+Z or Ctrl+Y: redo
B: brush                     E: eraser
F: flood fill                I: color picker
L: line                      R: rectangle
O: ellipse                   S: rectangular selection
T: text
Ctrl+A: select all           Ctrl+Shift+A: deselect
Delete: clear selected pixels (or the whole active layer without a selection)
Ctrl+Shift+X: crop all layers to the selection
Ctrl+C: copy active pixels   Ctrl+X: cut active pixels
Ctrl+V: paste image as a new layer
Escape: cancel the current stroke

Use Command instead of Ctrl on macOS.

Architecture
------------
src/document.rs: pixel storage, brush/fill operations, edit transactions and history.
src/files.rs: bounded raster/project decoding and atomic project/PNG saving.
src/transform.rs: pixel resampling, canvas anchoring and exact right-angle transforms.
src/app.rs: desktop workspace, input, dialogs, canvas texture and tool controls.
src/main.rs: native window startup.
tests/editor.rs: image operations, history semantics and file round trips.

The document stores up to 32 RGBA layers with configurable blend modes.
Brush strokes replace RGBA values on the active layer; repeated samples do not
accumulate opacity. Transparent pixels are displayed over a checkerboard. History
shares unchanged layer buffers and retains up to 128 MiB of unique pixel buffers
across undo/redo; history labels and metadata are additional allocations. Oldest
undo entries are evicted first, then farthest redo entries when necessary. The
active document is limited to 256 MiB of layer pixels, excluding previews/history.
A canvas is limited to 16,777,216 pixels. The New dialog permits up to 4096 per axis.

Save writes an editable .prust project. Export PNG writes only the visible composite
and does not mark an edited project as saved. Project version 2 stores blend modes as well as dimensions,
layer order, the active layer, UTF-8 names, visibility, opacity and lossless PNG
pixels for every layer. The reader bounds layer counts, decoded memory, payloads
and names, and rejects unsupported versions, truncation and trailing data. History
is session-local and is not stored in project files.

Current boundaries
------------------
This is an initial application, not a complete graphics suite. Pressure-sensitive input, advanced effects, autosave and session
recovery are not implemented yet. Image decoding, encoding and fill run on the UI
thread; large operations can pause the interface. Canvas updates upload the full
image texture. Metadata, orientation tags and embedded color profiles are not
preserved or interpreted. PNG is the only raster export format. There is no installer or
signed distribution yet. Linux is the currently validated desktop target.

Validation
----------
    cargo fmt --check
    cargo test
    cargo clippy --all-targets -- -D warnings

Tests cover continuous strokes, erase/cancel, fill boundaries, saved history
revisions, malformed input, PNG transparency and failed replacement cleanup.
Headless GUI tests exercise pointer drawing, Escape cancellation and the unsaved
changes gate. The native window should also be smoke-tested on the target desktop.

Initial validation (2026-10-24, Linux, Rust 1.99.1): 14 tests passed, formatting
and Clippy checks passed without warnings, and the optimized build completed.
The native OpenGL window was visually inspected on X11; the release executable
also passed a startup/event-loop smoke check. Wayland and other operating systems
still need separate desktop validation.

Desktop appearance
------------------
The workspace follows the system light/dark preference. On Linux an asynchronous
XDG Settings portal subscription reads the initial preference and receives changes
without blocking the UI. An unavailable preference falls back to light. GNOME's
configured interface font is resolved through Fontconfig at startup; bundled fonts
remain available when desktop font discovery fails. Restart after changing the
system font. Font sizes use logical pixels; the window backend supplies DPI scaling.

Native window decorations remain enabled. Linux builds include the Adwaita client
frame for Wayland compositors that do not provide server-side decorations. X11
continues to use the window manager's frame. Panels, buttons and workspace use
coordinated light/dark palettes, with a fixed blue accent. Widgets are rendered by
egui; arbitrary GTK themes and system accent colors are not imported.

Appearance validation: 16 tests pass, including the existing editing regressions,
font description parsing and light/dark palette switching. Formatting and Clippy
checks pass. The light interface and desktop font were visually checked on X11;
live desktop theme changes and Wayland decorations need visual validation in their
respective desktop sessions.

License and artwork
-------------------
Project-authored source and artwork are available under MIT OR Apache-2.0, at your
option. See LICENSE-MIT, LICENSE-APACHE and THIRD_PARTY_NOTICES. Dependencies retain
their own licenses. The original SVG tool/action icons and application icon are
in assets/; assets/README.txt records their provenance and design conventions.
Icons follow the current text color in light, dark, selected and disabled states.
Labels and shortcut hints remain available beside the icons.

Layer/project validation: 21 tests pass, including alpha/opacity/order, shared
buffer isolation, history, Unicode layer names, editable project round trips and
malformed project rejection.

Shape validation: 25 tests pass. Previews replace intermediate geometry, Escape
restores the starting layer, and pointer release commits its final position once.
Shape edges currently use pixel-center coverage without antialiasing.

Selection validation: 30 tests pass. Selection gestures do not dirty the document;
Escape restores the previous selection. Brush, eraser, fill, shapes, invert and
delete respect rectangular bounds. Crop affects every layer and restores geometry
and the selection on undo. Selections are session-local, not project data.

Geometry validation: 34 tests pass. Image resizing offers nearest-neighbor and
premultiplied-alpha bilinear sampling. Canvas expansion is transparent; anchor
selection controls cropping/placement. Rotation and mirroring preserve exact pixel
values. Geometry changes affect every layer and clear the selection; undo restores
its previous bounds. Resampling currently interpolates encoded RGB values rather
than linear-light colors, and bilinear downsampling is not an area filter.

Clipboard validation: 39 tests pass, including injected clipboard failures, native
cut/paste input events, selected-region copying and one-step paste undo. Cut only
deletes pixels after the clipboard accepts the image. Paste uses the selection
origin or centers the image, clipping to the canvas. Copy merged is in the Edit
menu. The clipboard connection stays alive while the editor runs. This backend
handles image paste on Ctrl+V release because the window integration consumes its
press for text paste. OS clipboard interoperability still needs validation across
target desktops; tests use an injected memory clipboard rather than replacing
the user's clipboard contents. Clipboard decoding is currently synchronous.

Text validation: 43 tests pass. Text uses the interface font, glyph coverage and
basic kerning, including Polish characters. The canvas preview stays pending until
Apply; Cancel/Escape restores the previous layer. Applied text becomes raster
pixels. Complex-script shaping, bidirectional layout, font fallback, persistent
text objects and font selection are not implemented. Size is in image pixels.

Blend validation: 46 tests pass. Normal, Multiply, Screen, Overlay, Darken, Lighten,
Difference and Add are available in Layer properties. Blend calculations use
encoded RGB with explicit source/backdrop alpha. Project version 2 preserves the
mode per layer; version 1 projects remain readable with Normal mode. Unknown
blend-mode identifiers are rejected rather than silently changed.

Adjustment validation: 51 tests pass. A dedicated preview worker coalesces requests,
cancels obsolete calculations and publishes only the latest generation. Apply is
disabled until that generation is ready. Cancel/Escape restores original pixels.
Effects use a frozen active-layer buffer and respect the selection. The blur uses
a separable box kernel and premultiplied alpha. Full layer compositing and texture
upload still happen on the UI thread; this is not yet the final performance path.
