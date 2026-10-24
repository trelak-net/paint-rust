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

Linux desktop installation
--------------------------
Build and install for the current user:

    cargo build --release --locked
    python3 scripts/install_linux.py

This installs the executable under ~/.local/bin and desktop metadata under
$XDG_DATA_HOME (default ~/.local/share). It adds a launcher, the original application
icon and the .prust MIME type. It does not change default file associations or
require root. Open With can select Paint Rust for supported images and projects.
Use --prefix /path/to/prefix for an isolated installation; --data-dir overrides
the metadata location. --no-refresh skips desktop and MIME database updates.
The prefix must not contain percent signs, equals signs, tabs or newlines.

Validate the installer without changing the user's desktop:

    python3 -m unittest discover -s scripts -p 'test_*.py'

The desktop launcher follows the Desktop Entry Specification:
https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html
This is a source-build installation helper, not a signed distribution package.
Full third-party binary distribution notices and cross-desktop release testing
remain outstanding. Re-running the helper replaces installed application files;
user projects and recovery data are kept separately.

Features
--------
- Create an opaque or transparent canvas with custom dimensions.
- Paint continuous round brush strokes; erase to transparency.
- Fill a contiguous area of exactly matching RGBA pixels.
- Pick a color from the canvas, edit RGBA values, or use palette swatches.
- Invert RGB while preserving alpha.
- Preview brightness/contrast, saturation, grayscale, sepia and alpha-aware box blur.
- Pixelate with alpha-aware block averages or posterize individual color channels.
- Draw lines and outlined/filled rectangles or ellipses with cancellable previews.
- Select a rectangular area, limit edits to it, delete pixels or crop all layers.
- Resize all layers, change canvas size with a nine-position anchor, rotate or mirror.
- Hold Shift for 45-degree line angles or equal shape dimensions.
- Zoom from 10% to 400%, fit the canvas, and navigate with scrollbars.
- Use Ctrl+wheel or a pinch gesture to zoom around the pointer; middle-drag to pan.
- Undo/redo complete strokes and edits; cancel a stroke with Escape.
- Open PNG, JPEG, BMP and WebP; export a flattened RGBA PNG.
- Add, duplicate, reorder, hide and delete layers; edit names, opacity and blend mode.
- Flatten the visible image into one layer, with undo restoring every original layer.
- Copy/cut active-layer pixels or copy the merged image; paste images as new layers.
- Import an image or a project composite as a named layer in the current document.
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
T: text                      M: move pixels/selection
G: gradient                  X: swap primary/secondary colors
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
This is an initial application, not a complete graphics suite. Pressure-sensitive
input, nonrectangular selections and free transforms are not implemented yet.
File decoding/encoding, recovery copies, effect previews and image/canvas resizing,
rotation and mirroring run in background workers. Compositing and fill still run on the UI thread;
large operations can pause the interface. Canvas updates upload the full
image texture. Metadata, orientation tags and embedded color profiles are not
preserved or interpreted. PNG is the only raster export format. A user-local Linux installation helper is available; there is no signed
distribution yet. Linux is the currently validated desktop target.

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

Geometry validation: 34 tests pass. Image resizing offers nearest-neighbor sampling,
premultiplied-alpha area reduction and bilinear enlargement. Canvas expansion is transparent; anchor
selection controls cropping/placement. Rotation and mirroring preserve exact pixel
values. Geometry changes affect every layer and clear the selection; undo restores
its previous bounds. Resampling currently interpolates encoded RGB values rather
than linear-light colors, and mixed-axis resizing (one dimension growing, one shrinking) uses bilinear sampling.

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

Move validation: 54 tests pass. Move uses a frozen source region, so overlapping
destinations cannot smear the input. Ctrl copies instead of clearing the source;
Shift constrains direction. Selection-only movement does not dirty image pixels.
Pixels outside the canvas are clipped. Escape and undo restore the prior pixels
and selection; zero-distance movement is an exact no-op.

Gradient validation: 57 tests pass. Linear and radial gradients interpolate
premultiplied RGBA between the primary and secondary colors, then composite over
the original active layer. Previews replace earlier drag results, respect the
selection and cancel without history. Gradient calculations are currently on the
UI thread and use encoded RGB interpolation.

Recovery validation: 62 tests pass. Unsaved committed changes are copied in the
background at approximately 15-second intervals. On Linux, recovery data lives in
$XDG_STATE_HOME/paint-rust/recovery (default ~/.local/state/paint-rust/recovery).
A startup dialog offers recovery from interrupted sessions; Recover remains
available after choosing Later. Recovered work must be saved to a project file.
Session locks exclude running instances, and discarded snapshots cannot reappear
when an older background write finishes. Pending tool previews are not copied.
Recent edits since the last completed copy can be lost after a crash. Recovery
does not overwrite project files and is not a substitute for regular saves.
Discarded snapshots are reclaimed after pending writes finish, or when the next
instance discovers an unlocked session. Empty session lock directories are retained.

Flatten validation: 65 tests pass. Flatten bakes visible layers, opacity and blend
modes into a single normal RGBA layer while preserving the exact composite. Hidden
layers are removed; Undo restores them, layer settings and the active layer. The
selection does not restrict flattening and is retained. Pending edits block it.

Import validation: 69 tests pass. Imported images use the file stem as the layer
name and preserve the current project path and canvas size. Placement follows the
selection origin, or centers smaller images without a selection; excess pixels
are clipped. Project imports use the visible composite. The complete insertion
is one undo step. Failed decoding or validation leaves the document intact.
Native file picking remains modal; import decoding runs in a background worker.

Additional effect validation: 72 tests pass. Pixelate averages premultiplied color
and alpha in blocks anchored to the canvas origin, including partial edge blocks.
Posterize quantizes each RGB channel to 2–32 levels while retaining alpha. Both
use cancellable background previews and leave unselected pixels untouched. As
with blur, pixelation may sample pixels outside the selection, but only writes
inside it. Invalid effect buffer dimensions produce no output.

Background file validation: 75 tests pass. Open, save, export, import and recovery
decoding run off the UI thread. Editing is temporarily disabled while a file job
is active; the window continues repainting a busy indicator. Closing waits for
the operation to finish. A Save-before-New/Open/Close action proceeds only after
a successful write; failures keep the document and unsaved-changes choice. Export
does not mark a project saved. Native file pickers remain modal. File operations
currently have no cancellation or byte progress.

Navigation validation: 77 tests pass. Pointer-anchored zoom preserves the image
coordinate beneath the pointer except where canvas edges clamp scrolling. Middle
button dragging pans without creating history. Fit uses the actual canvas panel
and resets its scroll offset. File loading and new canvases reset navigation.

Background geometry validation: 80 Rust tests pass. Resizing, canvas expansion,
rotation and mirroring compute against immutable shared layer snapshots. Successful
completion publishes one history entry in the original document; undo retains
earlier edits and the saved revision. Failed jobs leave the document unchanged.
Unchanged results retain selection and history. Geometry jobs currently block
editing and close requests, but keep the window repainting; cancellation remains
to be implemented. The installer has four additional Python checks.

Area reduction validation: 83 Rust tests pass. Smooth reduction with neither axis
growing integrates pixel coverage across each destination footprint. Fractional
edge coverage is weighted and colors use premultiplied alpha to prevent invisible
RGB from bleeding into the result. Nearest-neighbor remains available. Filtering
uses encoded RGB, not linear-light color.
