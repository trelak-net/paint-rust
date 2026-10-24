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
- Zoom from 10% to 400%, fit the canvas, and navigate with scrollbars.
- Undo/redo complete strokes and edits; cancel a stroke with Escape.
- Open PNG, JPEG, BMP and WebP; save RGBA PNG.
- Confirm unsaved changes before opening, creating or closing a document.
- Write through a temporary sibling file before replacing the destination.

Keyboard
--------
Ctrl+N: new image             Ctrl+O: open image
Ctrl+S: save PNG              Ctrl+Shift+S: save PNG as
Ctrl+Z: undo                  Ctrl+Shift+Z or Ctrl+Y: redo
B: brush                     E: eraser
F: flood fill                I: color picker
Escape: cancel the current stroke

Use Command instead of Ctrl on macOS.

Architecture
------------
src/document.rs: pixel storage, brush/fill operations, edit transactions and history.
src/files.rs: bounded image decoding and temporary-file PNG saving.
src/app.rs: desktop workspace, input, dialogs, canvas texture and tool controls.
src/main.rs: native window startup.
tests/editor.rs: image operations, history semantics and file round trips.

The document has one RGBA raster. Brush strokes replace RGBA values directly;
repeated samples do not accumulate opacity. Transparent pixels are displayed over
a checkerboard. History retains up to 128 MiB of full-image snapshots; older
entries are evicted first. The active image and in-progress stroke snapshot are
additional allocations. A document is limited to 16,777,216 pixels. The New dialog
allows dimensions up to 4096 on each axis.

Current boundaries
------------------
This is an initial application, not a complete graphics suite. Layers, selections,
text, shapes, pressure-sensitive input, advanced effects, autosave and session
recovery are not implemented yet. Image decoding, encoding and fill run on the UI
thread; large operations can pause the interface. Canvas updates upload the full
image texture. Metadata, orientation tags and embedded color profiles are not
preserved or interpreted. PNG is the only output format. There is no installer or
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
