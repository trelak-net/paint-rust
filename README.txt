Paint Rust
==========

Application/package identifier: net.trelak.PaintRust.

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
build, together with pkg-config and dav1d development headers (libdav1d-devel on
Fedora, libdav1d-dev on Debian-family systems). Dependencies and their exact resolved versions are recorded in Cargo.lock.
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
- Sharpen local RGB contrast while preserving alpha and the active selection.
- Draw lines and outlined/filled rectangles or ellipses with cancellable previews.
- Select a rectangular or elliptical area, limit edits to it, delete pixels or crop all layers.
- Add, subtract or intersect selections; combined areas use a blue mask overlay.
- Build polygon selections by clicking vertices; Enter/double-click finishes them.
- Resize all layers, change canvas size with a nine-position anchor, rotate or mirror.
- Free-transform selected pixels with rotation, scaling, mirroring and offsets.
- Hold Shift for 45-degree line angles or equal shape dimensions.
- Zoom from 1% to 6400%, fit the canvas, and navigate with scrollbars.
- Use Ctrl+wheel or a pinch gesture to zoom around the pointer; middle-drag to pan.
- Undo/redo complete strokes and edits; cancel a stroke with Escape.
- Use the History panel to undo or redo multiple operations at once.
- Open PNG, JPEG, BMP and WebP; export PNG, JPEG, BMP or lossless WebP with a decoded preview.
- Add, duplicate, reorder, hide and delete layers; edit names, opacity and blend mode.
- Flatten the visible image into one layer, with undo restoring every original layer.
- Copy/cut active-layer pixels or copy the merged image; paste images into the active layer, a new layer or a new image.
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
P: polygon selection         Enter: finish polygon; Backspace: remove last point
G: gradient                  X: swap primary/secondary colors
Move tool: arrows move 1 px; Shift+arrows move 10 px; Ctrl+arrows copy pixels
Ctrl+A: select all           Ctrl+Shift+A: deselect
Delete: clear selected pixels (or the whole active layer without a selection)
Ctrl+Shift+X: crop all layers to the selection
Ctrl+C: copy active pixels   Ctrl+X: cut active pixels
Ctrl+V: paste into the active layer
Ctrl+Shift+V: paste into a new layer
Ctrl+Alt+V: paste into a new image
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
Brush strokes support replacement or source-over blending, hardness, stroke
opacity and optional edge antialiasing. The eraser reduces alpha while retaining
RGB. Coverage accumulates once per gesture, so repeated samples do not accumulate
opacity. The temporary coverage mask uses one byte per canvas pixel. Transparent pixels are displayed over a checkerboard. History
shares unchanged layer buffers and keeps up to 128 MiB of unique pixel and
selection-mask buffers in RAM across undo/redo for all open documents. Older states spill into private
temporary files instead of being evicted. Labels and metadata remain in RAM. The
active document is limited to 256 MiB of layer pixels, excluding previews/history.
A canvas is limited to 16,777,216 pixels. The New dialog permits up to 4096 per axis.

Save writes an editable .prust or .pdn project. Raster export writes only the visible composite
and does not mark an edited project as saved. Project versions 2/3 store blend modes as well as dimensions,
layer order, the active layer, UTF-8 names, visibility, opacity and lossless PNG
pixels for every layer. The reader bounds layer counts, decoded memory, payloads
and names, and rejects unsupported versions, truncation and trailing data. History
is session-local and is not stored in project files.

Current boundaries
------------------
This is an initial application, not a complete graphics suite. Pressure-sensitive
input is not implemented yet. Free transforms provide numeric controls and on-canvas handles.
File I/O, recovery, raster previews, geometry, fill, clipboard operations and disk
history run in background workers with cooperative cancellation. Small brush edits
use changed-region compositing and partial texture uploads. Initial copies of
shared layer buffers, allocation and GPU uploads still take time on the UI thread;
this is not a guarantee of constant frame time at the document limits. PDN/PRUST
preserve string metadata and layer background flags. Raster EXIF orientation is applied to pixels on import, including mirrored
orientations. Embedded RGB/grayscale ICC profiles are converted to sRGB on raster
import. PNG/JPEG/WebP/TIFF exports embed sRGB; source descriptive metadata is
omitted. PNG, JPEG,
BMP, lossless WebP, TIFF and TGA are available for raster export. A user-local Linux installation helper is available; there is no signed
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

Image resizing offers eight sampling choices with optional linear-light gamma
correction and premultiplied-alpha filtering. Canvas expansion is transparent; anchor
selection controls cropping/placement. Rotation and mirroring preserve exact pixel
values. Geometry changes affect every layer and clear the selection; undo restores
its previous bounds. Mixed-axis resizing filters each dimension independently.

Clipboard validation: 39 tests pass, including injected clipboard failures, native
cut/paste input events, selected-region copying and one-step paste undo. Cut only
deletes pixels after the clipboard accepts the image. Paste uses the selection
origin or centers the image, clipping to the canvas. Copy merged is in the Edit
menu. The clipboard connection stays alive while the editor runs. This backend
handles image paste on Ctrl+V release because the window integration consumes its
press for text paste. OS clipboard interoperability still needs validation across
target desktops; tests use an injected memory clipboard rather than replacing
the user's clipboard contents. Clipboard decoding now runs in a background job.

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
a separable box kernel and premultiplied alpha. Layer compositing now runs in a background worker; texture upload remains on the
UI thread and still transfers the full image.

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
does not mark a project saved. Native file pickers remain modal. Project save and PNG export can be cancelled before atomic replacement; other
file operations currently have no cancellation or byte progress.

Navigation validation: 77 tests pass. Pointer-anchored zoom preserves the image
coordinate beneath the pointer except where canvas edges clamp scrolling. Middle
button dragging pans without creating history. Fit uses the actual canvas panel
and resets its scroll offset. File loading and new canvases reset navigation.

Background geometry validation: 80 Rust tests pass. Resizing, canvas expansion,
rotation and mirroring compute against immutable shared layer snapshots. Successful
completion publishes one history entry in the original document; undo retains
earlier edits and the saved revision. Failed jobs leave the document unchanged.
Unchanged results retain selection and history. Geometry jobs currently block
editing and close requests, but keep the window repainting and now support
cooperative cancellation. The installer has four additional Python checks.

Area reduction validation: 83 Rust tests pass. Smooth reduction with neither axis
growing integrates pixel coverage across each destination footprint. Fractional
edge coverage is weighted and colors use premultiplied alpha to prevent invisible
RGB from bleeding into the result. Nearest-neighbor remains available. Filtering
uses encoded RGB, not linear-light color.

Elliptical selection validation: 88 Rust tests pass. The selection tool offers an
elliptical mode; its mask limits painting, fill, adjustments, text, gradients and
delete. Copy uses transparent pixels outside the ellipse. Move preserves masked
pixels and the ellipse's original geometry when clipped at canvas edges. Undo
and cancellation restore the prior selection. Crop uses its rectangular bounding
box. Selection edges use pixel-center coverage without feathering. Rapid pointer
release gestures are finalized even before the UI recognizes a drag threshold.

Keyboard movement validation: 89 Rust tests pass. The Move tool accepts arrow
keys for pixel nudging and uses the selection-only toggle for bounds movement.
Each pixel-changing nudge is undoable; selection-only movement remains clean.
Text fields keep their own arrow-key handling.

Canvas worker validation: 91 Rust tests pass. Compositing and conversion to display
colors run on a dedicated worker with coalesced requests and cancellation checks.
Generation checks discard stale results. The canvas displays the last completed
preview while a new one is computed; results from an older image size are hidden.
Texture upload remains on the UI thread. A native release smoke test confirmed
canvas display and the net.trelak.PaintRust window class.

Save cancellation validation: 94 Rust tests pass. Project saving and PNG export
check cancellation during pixel conversion, encoding writes and before atomic
replacement. Cancellation keeps the previous destination and removes temporary
output. A cancelled Save-before-Close/New/Open returns to the unsaved-changes
choice. If replacement has already completed, the operation reports success.
Cancellation is cooperative; an in-progress encoder or filesystem call can take
time to reach its next checkpoint.

History panel validation: 95 Rust tests pass. The panel lists retained undo and
redo operations in traversal order and reports retained pixel and selection-mask memory.
Choosing an entry traverses every operation through that entry. Saved revisions
remain recognizable, and new edits invalidate the redo branch. History is bounded
by the existing memory budget, so evicted operations are unavailable.

WebP export validation: 97 Rust tests pass. The Export menu offers PNG and lossless
WebP with transparency. Both export the visible composite through a cancellable
background job and atomic replacement. Export does not change project layers,
history, path or saved state. Format-specific file dialogs validate the extension.

Combined selection validation: 104 Rust tests pass. Replace, Add, Subtract and
Intersect are available in selection-tool options. Combination runs in a worker
and publishes session-local masks without dirtying image pixels. With no prior
selection, Add starts from empty; Subtract and Intersect start from the canvas.
An empty result remains explicitly empty, so drawing, fill, delete and move cannot
affect the whole image accidentally. Combined selections use a translucent blue
overlay that is not exported. Mask buffers are shared and cropped to their bounds;
retained history masks count toward the 128 MiB budget. Undo/cancel, clipboard and
clipped movement preserve mask geometry. Crop uses rectangular bounds and erases pixels outside the selection mask.

Polygon selection validation: 108 Rust tests pass. Polygon input accepts 3–4096
vertices, supports concave and self-crossing paths with an even-odd fill rule, and
clips to the canvas. Backspace removes the last point; Escape leaves the previous
selection intact. Completion runs in a worker and supports the same combination
modes as rectangle/ellipse selection. Masks use pixel-center coverage without
feathering. The polygon tool includes a new original SVG icon.

Free transform validation: 114 Rust tests pass. Selection > Free transform pixels
opens a cancellable background preview for rotation, independent scales, offsets,
mirroring and optional source copying. It transforms around the selection bounds'
center (the canvas center without a selection). Mask holes remain excluded and
output is clipped to the existing canvas. Smooth sampling uses premultiplied-alpha
bilinear interpolation in encoded RGB; nearest-neighbor is also available. Apply
creates one undo entry and clears the old selection when pixels changed. Undo
restores pixels and selection. Identity and empty-selection transforms are no-ops.

Sharpen validation: 116 Rust tests pass. Sharpen applies an adjustable unsharp
mask using a 3x3 alpha-aware box reference. It preserves source alpha and hidden
RGB in fully transparent pixels. Zero amount is an exact no-op. Color operations
are grouped under Adjustments; blur, sharpening and pixelation are under Effects.
The entire layers panel scrolls so properties/history remain reachable in smaller
windows with long layer lists.

Transform handle validation: 119 Rust tests pass. Drag inside the transform box
to translate, a corner to scale, or the blue control to rotate. Shift constrains
translation to one axis, preserves the current scale ratio, or snaps rotation to
15-degree steps. The rotation control sits inside the box so edge selections can
reach it. Preview requests coalesce during dragging, and canvas gestures cannot
paint while the transform dialog is active. Numeric controls remain available for
small or clipped transform boxes.

Dependency notices
------------------
The Linux distribution includes an indexed copy of active Cargo dependency
notices and Rust standard library notices. Before packaging, run:
    python3 scripts/dependency_notices.py --check
After updating dependencies or the compiler, regenerate with the same command
without --check and review the changed licenses. See THIRD_PARTY_NOTICES for
scope, provenance and other-target limitations.

Accessibility
-------------
AccessKit publishes the native accessibility tree, including Linux AT-SPI
integration. Palette swatches, layer visibility controls and the canvas have
explicit labels; primary and secondary color controls reference their labels.
A regression test checks the emitted tree. The drawing surface still requires
pointer input for painting, and a complete screen-reader workflow audit remains
to be performed.
Validation: 120 Rust tests, strict Clippy and the dependency-notice check pass.

Flood fill responsiveness
-------------------------
Flood fill computes on a private document snapshot in a background worker. The
Working dialog offers cancellation, checked every 4096 visited pixels. Completed
fills preserve the selection and publish one undo step; cancelled snapshots are
discarded. A final completed result can win a late cancellation request.
Validation: 122 Rust tests and strict Clippy pass, including selected-area
background filling and cancellation rollback.

Background layer operations
---------------------------
Flatten Image and Invert Colors also run on private snapshots with cancellation.
Both preserve the current selection and existing undo history when the completed
result is published. Flattening composites visible layers before changing the
layer stack; cancellation during compositing leaves the original stack intact.
Validation: 124 Rust tests and strict Clippy pass, including background publication,
selection preservation, undo restoration and pre-cancelled document operations.

Large-image preview scheduling and measurements
----------------------------------------------
During an active stroke, a pending canvas composite is allowed to complete before
the next snapshot is requested. This avoids continuously cancelling slow previews
and copying a whole shared layer buffer on every pointer frame. Finishing a stroke
requests its latest state immediately.
Run CPU measurements with:
    cargo run --release --locked --example performance -- 4096
On the development machine, four 4096 x 4096 layers with three Multiply blends
took about 1.1 seconds to composite. A brush frame sharing a renderer snapshot
took about 23 ms, while the following frame without a new snapshot took less than
0.01 ms. These are CPU measurements, not GUI frame-rate guarantees. The original measurement used full-image compositing; incremental updates are
described below.
Validation: 125 Rust tests and strict Clippy pass, including preview scheduling
while a stroke is active and publication after it ends.

RAM and temporary-file history
------------------------------
Undo and redo retain complete document states, including exact RGBA bytes, layer
properties, active layer, dimensions, selection geometry/masks and saved revisions.
When the 128 MiB RAM target is exceeded, older states move to temporary files in
$XDG_CACHE_HOME/net.trelak.PaintRust/history (default:
~/.cache/net.trelak.PaintRust/history). The cache is used instead of /tmp because
/tmp can be backed by RAM. Choose a disk-backed XDG_CACHE_HOME when necessary.
On Windows the base directory is LOCALAPPDATA; Windows runtime validation remains
outstanding. The history panel shows RAM and disk usage separately.

Files are private and automatically removed when their history entries are dropped
or the document closes. On Linux they are unlinked while open, so a process crash
also releases them. Undo history is session-only; recovery snapshots remain the
separate mechanism for recovering unsaved documents after a crash.

A failed write keeps the entry in RAM and reports the error, allowing RAM use to
exceed its normal target rather than silently losing undo. Reads validate file
length and a checksum before changing the document or either history stack. A
failed read leaves the current edit and history untouched, allowing retry.
Branching after undo releases obsolete redo files. The desktop workspace caps compressed undo payloads at 1 GiB;
large sessions require free cache space. Stored snapshots are full states compressed losslessly with zlib. The desktop editor
stores them and restores disk-backed history in background jobs. RAM-only undo
remains immediate. Editing is disabled while a history job is pending; the window
stays responsive. The library API remains synchronous unless storage is deferred.

Validation: 131 Rust tests and strict Clippy pass. Tests cover spilling beyond the
RAM budget without losing steps, exact restoration of layers/masks/revisions,
geometry and redo, failed writes, corrupted/truncated reads and redo cleanup. The
4096 x 4096 four-layer performance example now retains all three edit steps and
successfully undoes and redoes flattening instead of losing its entire history.

Background history I/O
---------------------
The editor defers history writes until a completed edit and performs storage on a
worker clone. Disk files are shared safely between clones; the original document
is retained until successful publication. RAM can temporarily exceed the target
while work is pending. Disk-backed multi-step undo/redo is cancellable between
steps and publishes all requested steps together. A read error or cancellation
keeps the original document. Failed storage remains in RAM; the History panel
provides Retry history storage instead of retrying continuously. Temporary render
and effect snapshots defer storage to avoid writing histories that are discarded.
Validation: 133 Rust tests and strict Clippy pass, including concurrent reads of
shared history files and background storage/navigation beyond the RAM limit.

Compressed history storage
--------------------------
Temporary history files use streaming zlib compression with a fast setting. Reads
validate the compressed stream, exact decoded size and pixel checksum. On the
four-layer 4096 x 4096 uniform-image benchmark, three states used about 3.8 MiB
instead of 768 MiB. Compression ratios depend on image content; noisy images can
remain close to their original size. Unchanged buffers are shared across stored states as described below.
Validation: 134 Rust tests, strict Clippy and the dependency-notice check pass;
round trips cover varied pixel bytes, transparent pixels and compressed corruption.

Incremental canvas updates
--------------------------
The compositor compares immutable layer snapshots in the worker, finds changed
rows and composites only their bounding rectangle. The UI uploads that rectangle
with a partial texture update. Geometry, layer visibility, opacity, blend-mode or
layer-count changes trigger a full refresh. Cancelled or superseded results cannot
be used as patch bases: the worker checks which generation the UI consumed.
On the four-layer 4096 x 4096 benchmark, compositing a 64 x 64 region took about
0.29 ms versus 1.1 s for the whole image. This excludes difference scanning and
GPU upload time. Large edits still use large rectangles; this is not tile storage.
Validation: 137 Rust tests and strict Clippy pass, including patch reconstruction,
stale-base protection and regional/full compositing parity for all blend modes.

Runtime accessibility audit status
----------------------------------
The runtime audit was performed after the other roadmap tasks. Manually starting
at-spi2-registryd on the isolated accessibility bus resolved the previous registry
activation failure; the application exposes 77 accessible nodes without an
AccessKit worker panic. The audit identified missing labels on numeric controls.
Brush size, zoom, image dimensions, transform offsets, layer names and text inputs
now have explicit labels; the accessible main window also has its document title.

Verified through native AT-SPI actions: New, labelled Width/Height controls, Create,
Add layer, Undo, Layer properties and Cancel. The canvas, palette and layer
visibility controls retain their existing labels. This does not establish full
screen-reader workflow support or spoken announcements.

The remaining audit is blocked at the native Open file chooser in an isolated
D-Bus session. Nautilus reports that org.gnome.Mutter.ServiceChannel has no owner,
and the portal cannot obtain the desktop window list. No accessible file chooser
appeared during the bounded check. Complete open/save and spoken-feedback testing
in a full desktop session with a functioning portal and accessibility registry.
The user's desktop accessibility settings were not changed. This limitation does
not establish a file-picker failure in a normal desktop session.

The normal native release starts successfully with net.trelak.PaintRust window
class. Validation passes 169 Rust tests, strict Clippy, six installer/package tests
and the dependency-notice consistency check.

Shared history buffers
----------------------
Disk history stores each distinct shared layer or selection-mask buffer once,
with separate metadata for every state. Disk usage counts unique files rather
than counting shared buffers once per state. Restored states reuse live decoded
buffers; weak caches retain neither pixel data nor otherwise unused files. Failed
multi-buffer writes retain the original snapshot and release newly created files.
The four-layer 4096 x 4096 benchmark retains three edits in about 1.9 MiB rather
than 3.8 MiB. Independently allocated buffers with identical contents are not
content-deduplicated. Files are released after the last referring state disappears.
Validation: 140 Rust tests and strict Clippy pass, including shared layer/mask
storage, undo/redo, partial-write failure and obsolete redo-file cleanup.

Cancellable image geometry
--------------------------
Image resize, canvas resize, rotation and mirroring now offer cancellation in the
Working dialog. Kernels check cancellation between rows and periodically within
wide rows; area downsampling also checks while visiting source pixels. Cancelling
after a layer has been computed restores the whole original transaction, including
selection, dimensions, active layer and history. A result already completed when
cancellation arrives may still be published.
Validation: 144 Rust tests and strict Clippy pass, including rollback after the
first layer, cancellation from another thread and guarded background publication.

Native Linux release archives
-----------------------------
Run `python3 scripts/package_linux.py` to build a native release and create a
.tar.gz archive plus its SHA-256 sidecar under dist/. The packager verifies pinned
dependency notices, resolves the executable from Cargo's artifact report and
checks that the build host can resolve its linked libraries. BUILD_INFO.json
records the source revision, dirty state, compiler, target, libc and binary hash.
The archive contains licenses, icons, desktop integration, internal checksums and
a standalone installer: extract it and run `python3 install.py`, or run
`./bin/paint-rust` directly. See INSTALL.txt for installation options.
This build uses system libraries and graphics drivers; compatibility with other
distributions has not been established. Checksums are not publisher signatures.
Archive serialization is deterministic for identical inputs and timestamp; this
does not claim that independent compiler builds produce identical binaries.
Validation: six Python tests pass, including installation from an extracted
archive, checksum verification, deterministic archives and unsafe-path rejection.
The actual release archive was extracted, verified, installed into an isolated
prefix containing spaces, and launched with a visible net.trelak.PaintRust window.

Cancellable file loading
------------------------
Opening images and projects, importing layers and recovering projects now expose
a Cancel action. Input reads are bounded to 64 KiB, seeks check cancellation, and
project decoding checks between layers and during pixel conversion. Import
compositing is also cancellable. The current document, path and undo history stay
intact when loading is cancelled. Codec computation between input reads may delay
cancellation; this is cooperative cancellation, not forced thread termination.
Validation covers pre-cancelled inputs, cancellation between read chunks, seek
rejection, project/raster pixel round trips and guarded UI publication.

Background cropping
-------------------
Cropping now runs in the cancellable geometry worker. It copies the current
selection into the worker snapshot, checks cancellation between rows and layers,
and publishes one undoable edit only after every layer is complete. Undo restores
the original dimensions and selection. Existing crop semantics are unchanged:
masked selections crop to their rectangular bounds and erase excluded pixels on
every layer, including hidden layers.
Validation: 150 Rust tests and strict Clippy pass, including selected-region
publication, undo restoration and preservation of pre-cancelled documents.

Background pixel deletion
-------------------------
The Delete command runs in the document worker and supports cancellation every
4096 pixels. Cancellation rolls back partial changes; successful completion
retains the selection and creates one undo step. Cutting still deletes pixels
only after the clipboard accepts the image. Validation: 151 Rust tests and strict
Clippy pass, including masked deletion, UI publication and cancellation.

Cancellable selection masks
---------------------------
Polygon rasterization and selection union, subtraction and intersection now offer
cancellation in the Working dialog. Computation checks between rows and while
packing the resulting mask. Cancellation retains the previous selection, without
changing pixels or creating history. Validation: 153 Rust tests and strict Clippy
pass, including all pre-cancelled combination modes and guarded UI publication.

Background gradient gestures
----------------------------
Linear and radial gradient previews use a coalescing worker. New pointer positions
cancel obsolete requests; unchanged positions do not restart work. Releasing the
pointer commits the latest completed preview, or waits for the final generation
in the cancellable Working dialog. Escape discards the entire gesture. Commands
that could alter the document are disabled during the drag, as for selection
gestures; the canvas keeps accepting pointer movement. One completed gradient
creates one undo step. The pixel kernel checks cancellation between rows and
within wide rows, retaining premultiplied color interpolation and selection masks.
On the 4096 x 4096 benchmark, linear and radial kernels took approximately 478 ms
and 557 ms respectively; these computations now run outside the UI thread.
Validation: 157 Rust tests and strict Clippy pass, covering final-generation
publication, undo, Escape during a real gesture, invalid input and cancellation.

Background shape gestures
-------------------------
Lines, rectangles and ellipses share the gradient gesture worker. Shift constraints
and the release endpoint are preserved; intermediate requests are coalesced and
obsolete renders are cancelled. The final result is published as one undoable
edit, with cancellation available while it finishes. Shape kernels and the shared
line/brush rasterizer check cancellation between rows and within wide rows.
Validation: 158 Rust tests and strict Clippy pass, including the real release
endpoint, Escape for every shape tool, selection masks, outlines, clipping and
invalid inputs. Existing brush and eraser coverage tests also pass.

Row-bounded brush rasterization
-------------------------------
Brush, eraser and line coverage now restrict candidate pixels to the segment's
horizontal span in each row's radius band. The original capsule coverage test
still decides which pixels change; a conservative numeric margin protects edges.
For a 12-pixel diagonal across 4096 x 4096, the warmed brush-frame CPU time fell
from about 36.3 ms to 0.36 ms on the same machine. The initial copy of a shared
64 MiB layer still costs about 22 ms and is reported separately by the benchmark.
Validation: 159 Rust tests and strict Clippy pass, including full-canvas reference
coverage for reversed, clipped, horizontal, vertical, zero-length and wide strokes.

Background pixel movement
-------------------------
Moving and copying selected pixels, including keyboard nudges, now use the
cancellable gesture worker. Rendering keeps the original source immutable and
checks cancellation while clearing and placing pixels. The final pixel buffer and
translated selection are published together; undo and redo restore both. Moving
only selection bounds remains immediate because it changes metadata, not pixels.
Validation: 161 Rust tests and strict Clippy pass, including overlapping movement,
masked coverage, latest-generation publication, nudges and Escape during dragging.

Background text previews
------------------------
Text content, size and color changes now use the coalescing preview worker. Apply
is enabled only after the latest request has been displayed. Escape and Cancel
discard the transaction. Closing the main window keeps the text dialog open so
the draft can be applied or cancelled explicitly. Cancellation is checked between
characters and during glyph coverage callbacks; an in-progress glyph rasterizer
may finish its internal work before the worker returns.
Validation: 164 Rust tests and strict Clippy pass, including stale text requests,
Unicode rendering, cancellation, close guarding and preservation of unapplied text.

Background clipboard operations
-------------------------------
Copy, Cut and Paste prepare pixels and access the native clipboard on a worker.
The shared clipboard owner stays alive between jobs. Merged Copy composites only
the selected bounds. Cut clears pixels only after a successful write; failure or
cancellation keeps the current document intact. Paste publishes the new layer
and selection together as one undo step. Native clipboard calls cannot be
interrupted internally: cancellation is checked before and after them. Cancelling
after a clipboard write may leave the copied image on the clipboard, while the
source pixels remain in the document.
Validation: 167 Rust tests and strict Clippy pass, including failed writes, a
blocked clipboard with responsive UI, cancellation before cut deletion, cut/paste
history, and regional/full compositing parity for every blend mode. Tests use
in-memory clipboard backends and do not replace the desktop clipboard contents.

Cancellable history I/O
-----------------------
History compression, decompression and checksumming now check cancellation in
64 KiB chunks and between shared buffers. Navigation checks before publication;
cancelling a partial multi-layer load leaves the current image and history intact.
Cancelling automatic history storage pauses it instead of immediately restarting
the same job. Undo remains in RAM above the usual soft budget until Resume history
storage is selected in the History panel. Opening or creating another document
resets the pause. Native file synchronization calls may still delay cancellation.
Validation: 169 Rust tests and strict Clippy pass, including cancellation after
one stored buffer, temporary-file release, unchanged navigation and pause/resume
behavior. Existing compressed corruption and shared-buffer tests remain passing.

Large-document and release audit
--------------------------------
The remaining long raster operations now use background workers: loading/import,
geometry/crop, fill/invert/delete/flatten, selection masks, shape/gradient/move/text
previews, clipboard operations and disk history. Cancellation checks guard result
publication and run within the main pixel or compression loops. Active brush
strokes use bounded row scans; compositing updates only changed regions when safe.
The 4096 x 4096 benchmark and its documented measurements are CPU observations on
one host, not cross-device frame-rate guarantees. First-write buffer copies and
full GPU uploads can still cause a short delay. Native codec, clipboard, glyph and
file-system calls may delay cancellation until they return to a checkpoint.
Validation: 169 Rust tests, six Python tests, strict Clippy and all 550 dependency
notice files pass. The current clean native release archive was checksum-verified,
installed into an isolated prefix containing spaces and launched successfully.
The visible application window and its net.trelak.PaintRust class were inspected.
AT-SPI screen-reader workflow validation remains the final roadmap item.

Pencil
------
The Pencil tool (N) draws a continuous one-pixel path without antialiasing,
independent of brush size. Left drag uses the primary color; right drag uses the
secondary color. It replaces RGBA pixels on the active layer, respects selections,
and records each gesture as one undo step. Escape cancels the current gesture.

Workspace and pointer conventions
---------------------------------
The tool panel uses compact labelled buttons and can be resized. Contextual options
appear above the canvas; the interactive history is shown once, below the layers.
Brush, Pencil, flood fill, lines, rectangles, ellipses and text use the primary
color with the left button and the secondary color with the right button. The
picker updates the corresponding color. Right-dragging a gradient reverses its
colors. Palette swatches accept left/right clicks for primary/secondary colors.
Eraser retains left-drag erasing; selection and movement retain left-button gestures.
Escape cancels a drawing gesture even while its initiating button remains held.

PDN3 interchange
----------------
Open and Save As accept .pdn layered documents. The bounded reader supports the
PDN3 container with a data-only NRBF object graph and deferred BGRA8 buffers,
including raw or gzip chunks, padded rows and legacy layer properties. Fourteen
blend modes, layer names, opacity, visibility, background flags and string
metadata are retained. The writer emits PDN3 with independently compressed chunks.
No serialized type is instantiated or executed.

Metadata-bearing .prust files use version 3; metadata-free files retain version 2.
Versions 1 and 2 remain readable. Metadata survives snapshots and recovery saves.
The codec observes the current 32-layer/256 MiB document limit, additionally
bounding object depth, record allocations, string sizes and decoded pixel blocks.
Cancellation is checked during graph I/O, pixel decoding and output writing;
file replacement remains atomic. Unsupported schemas return an error.

Validation includes independently authored PDN fixtures, an external Windows
re-save, all fourteen blend modes, metadata round trips, truncation, cancellation
and trailing-data rejection. A Windows serializer constructor probe accepts the
Rust writer's metadata arrays. The native Windows GUI opened and re-saved Rust-produced two-layer and fourteen-layer documents;
every layer pixel and property, including all fourteen blend modes, survived the return trip. This is not a claim of
complete PDN version compatibility.
Non-bitmap layers and future signatures
are not supported yet. PDN-specific ICC interpretation and metadata updates
after geometry changes remain separate work. Reproduce a codec round trip with:

    cargo run --example pdn_roundtrip -- INPUT OUTPUT.pdn

Multiple documents
------------------
New, Open and recovery create independent documents instead of replacing unsaved
work. Tabs retain each document's undo/redo, selection, filename, zoom and scroll
position. Each document has its own recovery session, including inactive tabs.
Ctrl+W closes the active document. Close all and application exit ask about each
unsaved document; cancelling leaves the remaining documents open. Switching is
blocked during background operations and unfinished text/effect transactions.
Closing the last document creates a clean blank workspace.

Document tabs show rendered thumbnails and support drag reordering. Ctrl+Tab and
Ctrl+Shift+Tab cycle in tab order; middle-click closes a tab with the normal
unsaved-change guard. Inactive rendering workers finish pending thumbnail updates.

Batch opening
-------------
The Open dialog accepts multiple files. Startup arguments and dropped files are
queued into independent tabs. A failed input leaves existing documents untouched,
does not prevent later files from opening, and remains listed in an error window.

Workspace panels
----------------
View offers independent Tools (F5), History (F6), Layers (F7) and Colors (F8)
visibility, floating panels, and layout reset. Ctrl+Shift+F5 through F8 reset the
corresponding panel position. Floating panels can be moved and resized; their
visibility, positions, sizes and floating mode are stored atomically in
net.trelak.PaintRust/workspace.txt under the user configuration directory.
Invalid settings fall back to the default docked layout. Floating tool icons retain
accessible names, shortcut tooltips and a readable current-tool label.
Two native launches with two PDN documents verified clean exit and stable panel
positions/sizes across restart, using isolated configuration and recovery folders.

Canvas navigation
-----------------
Zoom (Z): left-click zooms in, right-click zooms out, and dragging a rectangle
fits that region in the viewport. Pan (H): drag with the primary button. Hold
Space to pan temporarily from another tool; middle-button navigation remains
available. Zoom ranges from 1% to 6400%. View can enable a pixel grid, rendered
only at 800% or above and only over visible pixels. These operations do not edit
the document or add undo steps. Escape cancels an in-progress zoom rectangle.

Color controls and palettes
---------------------------
Color channels exposes numeric RGB, alpha, HSV and HEX RGBA editing for either
primary or secondary color. Reset colors restores black/white. Palette swatches
retain left/right assignment; Add current, Remove last, Reset, Load and Save As
manage up to 256 colors. The palette is saved atomically beside workspace settings.
Malformed or oversized palette files are rejected before replacing existing colors.

Drag a layer name onto another layer to move it to that position in the stack.
The selected layer stays selected, and Undo restores the previous order.

Layer thumbnails show the raw layer content over a transparency checkerboard.
They retain image proportions and remain visible for hidden layers.

Merge down (Ctrl+M) applies the active layer’s pixels, opacity and blend mode to
the layer below. The lower layer keeps its name, metadata, visibility, opacity
and blend mode. This includes hidden source layers and ignores the selection;
merging can change the overall appearance when lower-layer properties differ
from normal, fully opaque blending. The background operation supports Cancel
and records a single undo step.

Select color (W) selects connected pixels or all matching pixels in the image.
Tolerance ranges from exact matching (0) to all colors (255), measured as the
largest difference between premultiplied RGB and alpha channels. Fully transparent
pixels match regardless of hidden RGB. Sample merged image uses the visible
composite; otherwise sampling uses raw pixels of the active layer. Replace, Add,
Subtract and Intersect combine the result with the existing selection. Processing
runs in the background and can be cancelled without replacing the old selection.

Lasso select (Q): drag a freehand outline and release to close it. Escape cancels
the outline; a click retains the existing selection. The same selection combination
modes apply as for other selection tools. Outlines are limited to 4096 sampled
points; exceeding the limit cancels the gesture and retains the previous selection.

Flood fill shares tolerance, connectivity and sampling controls with Select color.
It always writes the active layer, even when sampling the merged image. Selection
boundaries block connected fills; global matching can fill disconnected selected
regions. Matching uses the original seed color throughout the operation.

Gradient shapes include Linear, Reflected, Radial, Diamond and Conical. Color
blending overlays the existing pixels; Replace writes the gradient RGBA directly.
Transparency preserves RGB while fading existing alpha from its original value
to zero. Right-button dragging reverses colors or the direction of the alpha fade.
All modes use the current selection and the cancellable gesture preview.

Destructive merge-down uses 8-bit intermediate alpha and channel values. Its
rounding is verified against externally saved PDN fixtures for all fourteen blend
modes, including partially
transparent pixels and hidden layers. The display compositor retains continuous
precision until the final output, so a merge can shift a channel by one level.

Text alignment positions each line relative to the clicked anchor: Left, Center
or Right. Letter spacing adjusts the gap between glyphs; line spacing scales the
font line height. Antialias text can be disabled for binary edge coverage. All
controls update the background preview; Apply remains one undoable text edit.

The text dialog can load a local TTF, OTF or TTC font file (the first face of a
collection). Font files are limited to 16 MiB and are read in a background worker.
Invalid files leave the current draft intact. The chosen font is reused for new
text within the current application session; font files are not bundled in projects.

Ctrl+F repeats the last applied adjustment or filter on the active layer using
the current selection. Ctrl+Alt+F reopens its settings. Each adjustment/filter
remembers its last-applied parameters during the session. Cancelling a dialog
does not replace these values. Geometric transforms and drawing gestures do not
replace the last repeatable effect. Repeating runs in a cancellable worker.

Exclusive or combines selections by keeping pixels in exactly one of the two
regions. With no previous selection, it starts from the new region. Ctrl+I inverts
the selection within the canvas; an absent selection represents the full canvas
for inversion, producing an explicit empty mask. Ctrl+Shift+I inverts pixel colors.

Hue / Saturation / Lightness rotates hue by up to 180 degrees, scales HSL saturation
from zero to twice its original value, and adjusts lightness toward black or white.
It affects selected RGB pixels while retaining alpha, including transparent pixels.

Levels provides input black/white, gamma and output black/white controls for
master RGB and individual channels. Master mapping is applied before channel
mapping. Output endpoints may be reversed to invert a channel. Auto levels
estimates ranges from selected pixels with nonzero alpha and optionally clips
up to 5% from each histogram tail; constant channels are left unchanged.
Both adjustments preserve alpha and participate in preview, undo and repeat.

Raster export opens a decoded-output preview with the exact encoded byte count.
JPEG offers quality 1–100; JPEG and BMP composite transparency onto a selectable
background color (white by default). PNG and lossless WebP retain alpha. Save export
writes the previewed bytes atomically; Cancel writes nothing. Export options are
remembered within the session. The reduced preview is capped at 512x320 pixels.
The project stays editable and its saved/modified state is unchanged by export.

Tonal curves
------------
Adjustments > Curves provides master RGB and separate red/green/blue curves.
Double-click the graph to add a point, drag to move it, or right-click to remove
an interior point. Numeric input/output controls and explicit Add/Remove buttons
provide keyboard editing. Each channel holds up to sixteen points with fixed
input endpoints, unique sorted inputs and shape-preserving cubic interpolation.
Master RGB is applied before per-channel curves. Alpha and pixels outside the
selection are preserved. Preview, cancellation, undo and session parameter recall
use the existing effect workflow. Luminance mode and reusable presets are pending.

Retouch tools
-------------
Clone stamp (C): Ctrl+click to bind a source to the current layer, then drag
to clone. Switching the destination layer does not change the source layer. Aligned source keeps the source/destination offset between strokes; turn
it off to restart at the source anchor for each stroke. A cross marks the source.
Source pixels are frozen at stroke start, so overlapping edits do not feed back.
Source coordinates outside the canvas are skipped. Source anchors belong to the
document in which they were set. Source identity survives reordering, edits and
undo; deleting the source rejects painting until it is restored or reset. Sample
merged image freezes the visible layer composite at the start of each stroke.
Recolor (J): left drag replaces secondary with primary; right drag reverses them.
Matching supports premultiplied or straight alpha. Recolor shifts all RGBA
channels by the source-to-target difference, clamps the result and composites
it over the original pixel with byte-quantized source-over blending.
Both tools use brush size, hardness, opacity, antialiasing and selection masks.
Clone supports source-over blending or replacement, including transparent samples.

Additional photo effects
------------------------
Gaussian blur uses a normalized separable Gaussian kernel with replicated canvas
edges. Motion blur supports distance, angle and centered/one-sided sampling with
bilinear interpolation. Both accumulate premultiplied color and alpha to avoid
leaking hidden RGB into visible edges; selection masks constrain the output.
Temperature/Tint adjusts warm/cool and green/magenta balance. Highlights/Shadows
uses luminance-weighted lightening or darkening. Both preserve alpha. Invert alpha
changes only alpha, including fully transparent pixels' retained RGB. All use
background preview, cooperative cancellation, undo and session parameter recall.
Exact external transfer-function and color-space comparisons remain pending.

Native validation after retouch tools: two isolated desktop launches and clean
closes preserved floating-panel layout. The tools palette now uses two to four
columns according to its width, with a wider and shorter default. A second pair
of launches and screenshot review confirmed all nineteen tools remain visible in
the default floating layout. Existing custom panel geometry is preserved.

Input histograms
----------------
Levels, Auto levels and Curves display an input histogram computed in a separate
worker from the frozen active-layer snapshot and current selection. Fully
transparent pixels are excluded; partially transparent pixels count once. RGB
view combines channel counts; single-channel views show their own counts. Linear
and logarithmic heights, exact hovered-bin counts and the selected pixel count
are available. Closing the dialog cancels unfinished histogram work. Output
histograms and external reference comparisons remain pending.

Exact tool colors
-----------------
Primary, secondary, text and palette colors retain unmultiplied RGBA bytes.
Numeric and HEX edits, sampling and palette persistence preserve hidden RGB at
zero alpha and channel values at low alpha. Color display converts to the UI's
premultiplied representation without changing stored values. The popup uses HSV
with independent alpha and updates the stored bytes only after a user edit.
Tests cover every alpha value, HEX/palette/HSV round-trips and idle UI frames.

Noise and stylization
---------------------
Add noise offers intensity, coverage, monochrome/color and a repeatable seed.
Median blur uses sliding alpha-weighted channel histograms with a configurable
percentile; it preserves each destination pixel's alpha. Edge detect computes
Sobel magnitude, while Emboss projects the gradient along an editable angle onto
a neutral-gray relief. Both preserve source alpha and use premultiplied samples.
These effects support selection masks, cancellable previews, undo and parameter
recall. External reference output comparisons remain pending.

TIFF and TGA
------------
Open/import and export dialogs support TIFF (.tif/.tiff) and TGA (.tga), including
decoded-output previews. TIFF exports one RGBA8 image; a multi-page input opens
its first image. TGA import uses the extension when no signature is available.
Tests verify all RGBA bytes, including hidden RGB, when encoding and reopening
a single raster, as well as cancellation and exact atomic output bytes.
Raster exports omit source descriptive metadata; TIFF embeds an sRGB ICC profile.
Codec options, multipage
editing and high-bit-depth working buffers remain pending. Dependency notices
were refreshed for the TIFF decoder/encoder and its active dependencies.

Installed-font browser
----------------------
The text dialog includes an Installed fonts section with a background Fontconfig
query, family/style search and face selection. The catalog is bounded to 4 MiB
and 16384 faces, with cancellation and a 15-second timeout. Font files retain the
16 MiB load limit. Available static styles and collection face indices are used
explicitly; named variable-font instances, bitmap fonts and Type 1 fonts are
excluded. Manual font-file selection remains available when Fontconfig is absent.
A native catalog probe found 321 outline faces and loaded the first twenty. Run
that environment-dependent check with cargo test --test system_fonts -- --ignored.

Unified Save and Save As
-----------------------
Save preserves the current filename for both layered projects and supported
raster formats. Save As offers PRUST, PDN, PNG, JPEG, BMP, WebP, TIFF and TGA.
Raster saves show the encoded preview and require confirmation of flattening.
After successful atomic output, the active document becomes the decoded raster
and is marked saved. One undo restores the previous layer stack and marks it
unsaved again. Export still writes a copy without replacing the document.
Cancel or failure preserves the document and its previous path; a pending close
does not proceed. Tests cover all six raster formats, decoded pixel identity,
undo/redo, cancellation and failed writes. Raster files omit document metadata;
retain a layered project separately when that information is required.

Preferences and recent files
----------------------------
View > Theme selects System, Light or Dark. An explicit theme overrides desktop
notifications; System follows the latest desktop preference. Normal shutdown
atomically saves the theme, pixel-grid visibility, brush size/hardness/opacity/
antialias/blending and up to sixteen recent successful open/save paths. Recent
entries can be reopened or cleared from the Recent menu. Paths with spaces,
Unicode and line breaks are encoded safely; unsupported non-UTF-8 paths still
open normally but are omitted from history. Preferences are size/range checked.
Two launches each at dark/100% and light/200% verified stable panel layout,
preferences and recent files. The native smoke script accepts --theme and --scale.
Further preference controls, localization and effect persistence remain pending.

Save all
--------
File > Save all (Ctrl+Alt+S) saves the active dirty document, then remaining dirty
documents in tab order. Each write must finish successfully before the next
document is activated. Untitled documents request a filename; raster documents
use the same flattening confirmation and preview as Save. Cancellation or failure
stops the sequence and leaves remaining documents unsaved. Tests cover multiple
project writes, a write failure and cancellation at a raster confirmation.
The File menu also groups basic open/save/close commands and recent files.

Pixel rulers and selection navigation
------------------------------------
View > Pixel rulers reserves separate nonpainting horizontal and vertical bands.
Ticks adapt to zoom and scroll, with major labels, minor divisions and pointer
markers. Visibility persists across restarts. Physical units and document print
resolution are not implemented yet. View > Fit image (Ctrl+B) fits the full canvas;
Fit selection (Ctrl+Shift+B) fits and centers selection bounds without editing
pixels. Tests verify tick spacing, nonpainting bands and selection fitting. Two
native launches with --rulers verified stable layout and preference persistence.

Shape catalogue and coverage
---------------------------
Shapes (U) offers eighteen geometries: line, rectangle, ellipse, rounded rectangle,
triangle, right triangle, diamond, pentagon, hexagon, octagon, star, four arrow
directions, trapezoid, parallelogram and heart. Rectangle/Ellipse shortcuts remain
available. Shapes support outline, fill and combined primary-outline/secondary-
fill modes; right drag swaps the colors. Optional signed-distance antialiasing
blends edge coverage in premultiplied color. Line antialiasing uses the continuous
brush kernel. Hard legacy rectangle/ellipse rendering remains available when
new options are off. Previews, selection masks, cancellation and one-step undo
are shared with the existing gesture workflow. Tests cover all geometries, reverse
drag, combined colors, hidden RGB, selection boundaries and secondary gestures.
Persistent handles, editable curves and exact reference coverage remain pending.

Distortion and photo corrections
--------------------------------
Effects are grouped into Blurs, Distort, Noise, Photo and Stylize. Bulge and Twist
use centered radial mappings; Frosted glass uses seeded disk scatter; Crystalize
uses seeded Voronoi cells. Sampling is bilinear with premultiplied alpha and
clamped canvas edges. Neutral settings preserve exact input bytes. Vignette
darkens image edges with an adjustable clear center; Red eye removal reduces
strong red dominance and should be restricted to the eye selection. Both retain
alpha. All six effects share cancellable previews, selection output masks, undo
and session parameter recall. Determinism, identity, alpha, selection and invalid
parameters are tested; movable centers and external output comparisons remain.

Color wheel
-----------
The Colors panel includes an expandable hue wheel and saturation/value field for
primary or secondary color, plus an alpha slider. The vector mesh is authored in
code for this project. Hue remains selected when saturation/value reaches zero,
so a new hue can be chosen before restoring color from gray or black. Numeric
HSV controls share that retained hue. Popup color pickers also retain transient
HSV state without quantizing stored RGBA during idle frames. Pointer tests verify
hue selection on black, subsequent saturation/value editing and alpha retention.
Numeric RGB/HSV/HEX controls remain available for keyboard input.

Persistent effect settings
--------------------------
Applied filter parameters and the last effect are restored from effects.json in
the configuration directory. Settings include curve points, levels and random
seeds. Version, size, numeric ranges and curve topology are validated before use;
invalid settings fall back to defaults. Writes replace the file atomically.
Geometry gestures are excluded. Tests cover malformed settings and round trips;
two isolated native launches verify persistence across clean application exits.

Exposure adjustment
-------------------
Adjustments > Exposure applies -10 to +10 EV gain in linear sRGB, then a
-1 to +1 linear offset and gamma 0.1 to 10 before encoding back to sRGB.
A bounded 256-entry table keeps preview work proportional to image size. Alpha,
hidden RGB in fully transparent pixels and pixels outside the selection remain
unchanged. Neutral settings preserve exact bytes. Parameters persist alongside
other filters; reference transfer-function comparison remains pending.

Additional blur paths
---------------------
Radial blur averages a centered angular arc, Zoom blur samples toward a movable
center, and Fragment blur combines evenly spaced offset copies. Centers use
normalized image coordinates; fragment distance is measured in pixels. Bilinear
sampling uses premultiplied alpha and clamps at image edges. Radial/zoom paths
use 65 samples per pixel; fragments use 2 to 32. Neutral settings preserve exact
bytes. All three support selection masks, cancellable previews, undo and saved
parameters. External reference output, configurable quality and large-image
performance comparisons remain pending.

Surface blur
------------
Surface blur averages nearby colors with Gaussian spatial weights and a linear
color-distance cutoff. The radius is bounded to 10 pixels; threshold 1 preserves
all distinct colors. Neighbor weights include opacity, while the destination
alpha and fully transparent RGB are retained. The filter supports the common
preview, cancellation, selection, undo and saved-parameter workflow. Reference
kernel comparison and large-image performance validation remain pending.

Raster color management
-----------------------
Raster import interprets embedded RGB or grayscale ICC profiles using Little CMS
and relative colorimetric intent, converting to the 8-bit sRGB working space.
Alpha is retained. Untagged images are assumed to be sRGB. Unsupported color
spaces, malformed profiles and profiles larger than 4 MiB produce an open error;
CMYK profiles are not applied to RGB samples produced by a decoder. Conversion
runs in bounded batches with cancellation. TIFF metadata has an independent
4 MiB read bound so small images can carry larger profiles.
PNG, JPEG, WebP and TIFF exports embed the working sRGB profile. BMP and TGA
remain untagged. Source EXIF/location/author/comment tags are omitted, and EXIF
orientation is applied to pixels once. Export preview shows the encoded bytes.
Tests cover linear RGB and grayscale conversion, alpha, tiny TIFF profiles,
invalid/unsupported profiles, sRGB identity and re-import without double conversion.
Display-monitor profiles, selectable rendering intents, high-bit-depth conversion,
CMYK import and PDN-specific profile metadata interpretation remain pending.
The color engine is statically linked; no additional runtime library is needed.
Binding API: https://docs.rs/lcms2/6.1.1/lcms2/

Text decorations
----------------
Text preview includes independent Underline and Strikethrough controls. Lines
follow each text line's advance width, alignment and spacing, including spaces.
They share the text color, antialias setting and selection clipping. Decoration
thickness and baseline offsets scale with font size; font-specific decoration
metrics remain pending. Tests cover multiple lines, alignment, clipping, partial
coverage and combined decorations. Apply and Cancel use the existing text transaction.

GIF images
----------
GIF import opens the first frame as a single raster layer. Save, Save As and
Export support a single-frame GIF with an automatically quantized palette of
up to 256 colors. Alpha below 128 becomes transparent; pixels at or above 128
are composited onto the selected matte and become opaque. The decoded preview
shows the actual palette result before writing. GIF remains untagged sRGB and
omits source descriptive metadata. Animation editing, palette selection and
dithering controls remain pending. Quantization is bounded by the document size,
but the codec does not expose cancellation within its palette-building pass.
Tests cover the alpha boundary, matte, first-frame import and cancelled writes.

Drop shadow
-----------
Effects > Object > Drop shadow blurs the source alpha, applies a selected shadow
color and opacity, offsets the result and composites the original pixels above
it. The output is clipped to the canvas and current selection. Radius is bounded
to 64 pixels and offsets to 200 pixels on each axis. Hidden source RGB cannot tint
the shadow; zero shadow opacity preserves exact bytes. Parameters are remembered,
and preview/cancel/undo follow the common filter workflow. External reference
edge behavior and large-image performance remain pending.

PDN metadata interoperability
----------------------------
The five-layer 17x13 metadata fixture includes Unicode names (Polish, Japanese
and an emoji), multiline string metadata, visibility/background flags, hidden
RGB and alpha/opacity boundary values. A native Windows re-save preserves every
layer byte and property. A second fixture renames the top layer in the native
properties dialog, preserves the remaining data and round-trips through this
editor again. tests/pdn.rs checks both external results; regenerate
the input with examples/pdn_metadata_probe.rs. This validates the supported PDN3
deferred-BGRA8 subset, not every historical container or pixel layout.

Luminance curves and presets
---------------------------
Curves offers RGB-channel and Luminance modes with separate retained curves.
Luminance uses weighted encoded RGB, looks up the new brightness and adds the
resulting difference to each channel. Channel differences are retained until
clipping; alpha and fully transparent RGB are unchanged. Presets provide Identity,
Increase contrast, Lift shadows and Invert for the displayed curve. Mode and
points persist across restarts; older settings default to RGB. Input and output
histograms follow the selected RGB/luminance mode. External transfer-function
comparisons remain pending.

Output histograms
-----------------
Levels, automatic levels and curves show input and output distributions. Output
bins are calculated in a background worker from the latest published preview,
with old workers cancelled when superseded. Changing parameters clears the old
output chart until new preview data arrives. RGB views combine three channel
counts; luminance counts each selected nontransparent pixel once using the same
weights as luminance curves. Both charts offer logarithmic height and hover counts.

Glow and portrait softening
--------------------------
Glow screens a Gaussian-blurred version over the original RGB with adjustable
strength and blurred-alpha weighting. Soften portrait mixes Gaussian detail
smoothing with the source and offers a warm/cool balance. Both retain source
alpha and fully transparent RGB, use the common selection/preview/undo workflow
and persist parameters. Zero-strength neutral settings preserve exact bytes.
Tests cover hidden colors, partial alpha, smoothing and warm/cool direction;
external reference algorithms and parameter-range comparisons remain pending.

Brush color blending
--------------------
Brush and Clone stamp offer all fourteen layer color blend modes when Blend color
is enabled. The mode is applied against the frozen pre-stroke destination, with
brush coverage and opacity counted once across repeated pointer samples. With
Blend color disabled, replacement painting remains unchanged. Eraser and Recolor
retain their existing behavior. The chosen mode is saved in preferences; older
preferences default to Normal and invalid mode indices are rejected. Tests compare
non-Normal brush results to layer compositing and cover Undo/Redo. A native dark
200% scaling check verifies the mode menu wraps without clipping and survives restart.

Menu and toolbar layout
-----------------------
The main menu occupies its own row, followed by common file/edit actions and
contextual tool controls. File > Export uses the shared codec inventory, including
GIF. Layers exposes shared layer actions and properties even with its panel hidden;
Edit includes Undo/Redo and Adjustments includes color inversion. Brush and zoom
sliders reserve their complete labels before wrapping. Tests cover clipping in
a narrow toolbar; native 800x600/light and 200%/dark checks cover startup and
persistence. Small workspaces temporarily use scrollable sidebars to keep
Tools, Colors, Layers and History reachable without overlapping floating windows.

Compact workspace fallback
--------------------------
Below 580 logical pixels of available canvas height, floating panels temporarily
use scrollable sidebars. All twenty tools use a compact icon grid. The floating
preference and saved window positions remain unchanged, and larger windows return
to that layout. Tests verify the preserved state and resize threshold; native
800x600/light and 200%/dark launches verify both layouts and clean restarts.

Window menu and help
--------------------
Window lists open documents in tab order, marks unsaved work and offers previous/
next navigation. Help > Guide and shortcuts, or F1, opens a scrollable English
user guide and shortcut reference. Escape closes help without altering the image
or selection. USER_GUIDE.txt is embedded in the executable and included in Linux
installation and distribution files. Tests cover non-destructive help shortcuts
and packaging. Polish localization and complete keyboard-navigation acceptance
remain pending.

Workspace undo memory budget
----------------------------
The desktop editor shares a soft 128 MiB undo/redo pixel-and-mask budget across
all open documents. When the aggregate exceeds this target, a background job
stores older states above each document's equal share, including inactive tabs.
It publishes the complete batch only after completion. Document pixels, paths,
views, saved revisions and the number of undo steps remain unchanged.
Cancelling storage retains all original histories in RAM and pauses automatic
storage for the entire workspace, including after switching or opening tabs.
Resume in History retries storage. A failed document remains in RAM with an error;
other documents can still spill. The panel reports aggregate undo RAM usage.
This is not a total process-memory limit: current pixels, previews, textures,
metadata and temporary worker allocations are separate. Compressed undo payloads
share a 1 GiB disk budget across documents. Filesystem allocation overhead, project
files and recovery snapshots are separate. Reaching the disk budget keeps all
remaining undo states in RAM and reports the reason. Closing documents or dropping
redo states releases their files; Resume history storage retries explicitly.
Regression checks cover inactive histories, exact undo/redo, guarded publication,
cancellation after a private partial batch and persistent workspace pause.

Procedural render effects
-------------------------
Effects > Render contains Clouds, Turbulence, Mandelbrot fractal and Julia fractal.
Clouds combines eight octaves of smooth value noise; Turbulence combines absolute
signed noise. Scale, roughness and seed are editable. Fractals expose iteration
count, center, zoom and rotation; Julia also exposes the complex constant.
Both colors, including alpha, are editable. Color interpolation uses premultiplied
alpha to avoid hidden-color fringes. Generators replace selected pixels, including
transparent pixels; areas outside the selection remain byte-identical.
Preview/cancel, one-step undo, repeat and persisted parameters use the shared
effect workflow. Iterations are limited to 512 and workers check cancellation
inside iteration loops. Tests cover seed stability, known interior/exterior
points, conjugate symmetry, palette alpha, selections and the complete preview/
cancel/apply/undo/repeat workflow. Exact external algorithms, additional palettes
and large-image performance comparisons remain pending.

Additional distortions
-----------------------
Effects > Distort includes Dents (seeded smooth displacement), Polar inversion
(adjustable center and signed amount), and Tile reflection (periodic displacement
with size, strength and rotation). Sampling is bilinear in premultiplied alpha;
coordinates outside the canvas replicate edge pixels. Zero strength preserves
all bytes, including hidden RGB. The inversion center remains fixed rather than
sampling an undefined radial direction. Selection clipping, preview/cancel, undo,
repeat and persistent settings use the common effect pipeline. Tests cover exact
neutral output, known coordinate mappings, deterministic seeds, constant colors,
hidden-color isolation, single-pixel images and invalid parameters. External
kernel/range comparisons and additional edge modes remain outstanding.

Morphology
-----------
Effects > Distort > Morphology offers Dilate, Erode, Open and Close with a square
neighborhood and radius 0–64. Operations take component extrema in premultiplied
RGBA, so color and opacity change together without leaking hidden RGB. Open runs
erosion then dilation; Close reverses the order. Canvas edges replicate pixels.
Horizontal/vertical sliding extrema keep computation linear in image size rather
than proportional to the neighborhood area. Workers check cancellation during
conversion and each pass. Radius zero preserves exact bytes. Regression checks
compare optimized results with direct neighborhoods on varied partial-alpha
pixels, and cover isolated defects, selected-area publication and bounds. More
structuring elements, channel options and external comparisons remain pending.

Noise reduction, outlines and relief
------------------------------------
Reduce noise blends an alpha-weighted neighborhood median with source RGB.
Outline converts local RGB range to dark contours on a white background; radius
and intensity control detail. Relief adds directional Sobel luminance shading to
source color, retaining its channel differences until clipping. All three preserve
source alpha and hidden RGB, support selection clipping and persist parameters.
Tests cover isolated noise, strength blending, flat areas, edge contrast, opposite
lighting directions, hidden pixels and parameter validation. Exact external
algorithms and large-image measurements remain pending.

Artistic effects
-----------------
Effects > Artistic provides Oil painting, Pencil sketch and Ink sketch.
Oil painting averages the dominant alpha-weighted tonal group in a square
neighborhood. A sliding histogram avoids rebuilding each neighborhood; radius
and bin count control detail. Pencil sketch uses a luminance color-dodge ratio
against Gaussian blur and blends its strength with the source. Ink sketch adds
thresholded Sobel contours with an adjustable amount of source color.
These effects retain source alpha and hidden RGB. Selection, cancellation,
preview, undo, repeat and parameter persistence follow the common workflow.
Tests cover weighted dominant colors, transparent neighbors, monochrome detail,
flat regions, edge thresholds and invalid settings. Exact external algorithms,
edge quality comparisons and large-image performance remain acceptance work.

Color quantization
-------------------
Effects > Color > Quantize reduces visible RGB colors to a maximum of 2–256 while
retaining each pixel's alpha and all hidden RGB. Images already within the target
color count remain byte-identical, even when close colors share a histogram cell.
A bounded 32×32×32 alpha-weighted histogram feeds deterministic weighted median
cuts; occupied cells map to the nearest palette RGB. This bounds palette search
work independently of full image size. The palette is derived from the active
layer; selection limits publication, not palette analysis. Dithering and palette
scope controls remain pending. Tests verify palette bounds, repeatability, exact
small-palette preservation, alpha/hidden colors, selection and cancellation.

Bokeh blur
-----------
Bokeh blur averages paired, evenly distributed disk samples with bilinear,
premultiplied-alpha sampling. Radius controls the disk; Gamma boost changes RGB
averaging and Quality controls 33–513 samples. Edges clamp to the canvas. Gamma
does not alter alpha averaging. Zero radius preserves exact bytes. Workers check
cancellation every 16 pixels. Tests cover constant images, point symmetry, hidden
colors, gamma brightness, selected-area publication and invalid values. Native
sampling parity, alternate edge modes and high-quality large-image performance
remain pending.

Straighten
-----------
Effects > Photo > Straighten rotates the active layer around the canvas center
from -45 to +45 degrees. Fill frame after rotation computes the zoom needed to
keep the frame filled; canvas dimensions stay unchanged. Without this option,
newly exposed corners are transparent. Sampling uses premultiplied bilinear
interpolation. Zero angle preserves exact bytes, and selection limits output.
Tests cover opaque corners in landscape/portrait/thin images, rotation direction,
center stability, selection, cancellation and bounds. An interactive horizon
line and external interpolation/range comparisons remain pending.

Sketch blur
------------
Sketch blur estimates RGB percentiles from seeded disk samples using a five-marker
P² streaming estimator. Radius sets neighborhood size; Smoothness increases the
sample count from 17 to 257, including the current pixel. Fully transparent
neighbors are excluded; visible samples have equal quantile weight. Source alpha
and hidden RGB remain unchanged. Zero radius is exact, and selection limits output.
The estimator follows the mathematical piecewise-parabolic algorithm described at
https://aakinshin.net/posts/p2-quantile-estimator-intro/; marker targets are computed
from the observation count to avoid accumulated position drift. Tests cover known
quantiles, marker ordering, constants, endpoints, seed stability, selections and
cancellation. External sampling/iteration parity, partial-alpha weighting and
large-image performance remain pending.

Selection-boundary transforms
-----------------------------
Selection > Transform selection boundary (Ctrl+Alt+M), also available from Move
selection options, transforms the mask without changing image pixels. Applying
the preview records one undoable selection change and keeps the saved image clean.
Scale X/Y, proportional scaling, rotation, flips and pixel offsets use the original
selection center. A coalescing background worker cancels superseded requests and
publishes only the latest generation. Apply/Enter keeps the ready preview;
Cancel/Escape/window close restores the exact original mask. Switching tabs,
image undo, file operations and automatic history storage are blocked during the
preview. The boundary uses binary nearest sampling and clips to canvas bounds;
an off-canvas result remains an explicit empty selection. Tests cover geometry,
ellipses and irregular masks, clipping, cancellation, stale results, tab guards,
Apply and image-pixel/saved-revision/path preservation. On-canvas handles and antialiased
selection coverage remain pending.

Layer orientation and selected-area fill
----------------------------------------
Layers includes exact horizontal/vertical flips and a 180-degree rotation for the
active layer. Other layers, properties, canvas dimensions and selection geometry
remain unchanged; these commands act on the entire active layer. F4 opens layer
properties. Edit provides primary/secondary Fill selection commands, replacing
RGBA in the mask (or the entire layer when no selection is active).
All raster work runs in cancellable background jobs and publishes as one undoable
edit. Tests cover exact pixel permutations including hidden RGB, inactive layers,
properties, selection retention, no-op history, cancellation and saved revisions.

Paste into a new image
----------------------
Edit > Paste into new image (Ctrl+Alt+V) reads the image clipboard in a background
job and creates an unsaved document at the exact clipboard dimensions. RGBA,
including hidden RGB, is preserved. Existing dirty documents remain in their
own tabs with paths and histories intact. Invalid clipboard buffers and cancelled
reads never install a tab. Tests exercise the native release-event shortcut,
partial alpha, tab preservation, interrupted reads and invalid buffers.

Selection geometry clipboard
-----------------------------
Copy selection geometry (Ctrl+Alt+Shift+C) writes polygon-list JSON text. Paste
selection geometry supports Replace (Ctrl+Alt+Shift+V), Add, Subtract, Intersect
and XOR. Rectangular runs merge vertically during export, preserving exact binary
masks including holes. Import uses even-odd filling across all polygons and clips
to the current canvas. Inputs are bounded to 8 MiB, 16,384 polygons and 81,920
vertices; overly complex exports report an error before replacing the clipboard.
Image pixels remain unchanged by geometry clipboard operations. Pasting records
one undoable selection change without changing the saved image revision. Workers check
cancellation during parsing, scan conversion and combination. Tests include
ellipses, holes, malformed text, publication guards and the complete clipboard
flow. An independently executed Windows clipboard round-trip preserves an
elliptical ring; native inversion produces its exact pixel complement. Fixtures
and provenance live in tests/fixtures/selection/. Copy merged also supports
Ctrl+Shift+C alongside the menu command.

Selection history
------------------
Rectangle/ellipse gestures, lasso/polygon/color selection, selection combinations,
inversion, select-all/deselect, boundary moves/transforms and geometry clipboard
paste now participate in undo/redo. Selection-only commits retain the saved image
revision, so selecting or deselecting does not mark a clean image as unsaved.
Preview cancellation and unchanged selections add no history entry. Selection
steps interleave with pixel edits and survive disk-backed history storage.
Ctrl+D deselects; Ctrl+Shift+A remains an alternative. Regression checks cover
single-step gestures, mixed image/selection history, saved revisions, branching,
preview cancellation, clipboard modes and disk restoration.

Paste destinations and fill shortcuts
-------------------------------------
Ctrl+V now replaces RGBA on the active layer. Ctrl+Shift+V creates a new layer;
Ctrl+Alt+V creates a new image. The pasted rectangle becomes selected and Move
pixels activates. Clipboard pixels, including transparent RGB, are copied exactly.
Existing selection geometry determines placement but does not clip the pasted
rectangle. Without a selection, placement is centered. When pasted pixels extend beyond the canvas, a confirmation offers expansion,
explicit clipping, or cancellation. Expansion preserves existing layer positions
and publishes the resize and paste as one undoable operation.
Paste is one undoable operation that restores both pixels and the prior selection.
Backspace fills with primary color; Shift+Backspace uses secondary. Polygon point
editing and text fields retain their own Backspace behavior. Tests cover each
paste shortcut, active-layer replacement, layer limits, alpha, clipping, undo and
both fill colors.

Erase and cut selection behavior
-------------------------------
Delete clears selected pixels to transparent black; Cut copies their original
RGBA before replacing them with transparent white. Both deselect on successful
publication. Undo restores the pixels and boundary together, including a
selection-only history entry when the pixels already match the cleared value.
Clipboard failures and cancellation preserve the original document.

Batch session recovery
----------------------
Recovery can open all discovered snapshots sequentially as separate unsaved tabs.
It preserves existing documents and adopts each successful recovery copy until a
replacement snapshot or explicit save is safe. Corrupt copies remain on disk and
in the recovery list while other candidates continue. Cancellation stops the
queue; summary status reports restored and failed counts. Versioned sidecar metadata preserves tab order within each workspace, file
destinations/titles, active tab and zoom/pan. Older snapshots or malformed/missing
sidecars still recover their pixels with default view settings. Only unsaved
documents have recovery snapshots; clean tabs are not reopened automatically.
View metadata is captured on the recovery interval and may lag the last gesture.
View-only changes write the small sidecar without rewriting image pixels.
Snapshots and sidecars are each atomically replaced; an interrupted pair can
retain an older view alongside newer pixels. Restored documents remain dirty.

DDS textures
------------
DDS import reads the top surface of legacy RGB16/24/32, BC1/BC2/BC3, and supported
DX10 RGBA8/BGRA8/BGRX8 or BC1/2/3 textures. BC4/BC5 UNORM supports
legacy ATI1/ATI2 and BC4U/BC5U as well as DX10 headers; channels map to red
and red/green respectively, with blue zero and opaque alpha. Signed variants
remain unsupported. Channel masks, row pitch, dimensions,
alpha conventions and input bounds are validated. BC1 transparent indices and
partial edge blocks are preserved. Cube maps, arrays, volumes, premultiplied alpha
and other block formats are rejected. Mipmap editing and regeneration are pending.
DDS Save/Export writes one uncompressed RGBA8 surface with exact channel bytes,
including hidden RGB. Decoded export preview and undoable raster Save apply.
No embedded ICC or source descriptive metadata is written. Synthetic independent
ImageMagick fixtures verify BC1/BC3 decoding with exact alpha and RGB error <= 1.
Format references:
https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header
https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-pixelformat
https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression

Undo disk quotas are enforced while compressed bytes are written, before a
snapshot replaces its RAM state. Tests exercise exhaustion within a multi-state
history, zero quota, cross-document sharing, full undo/redo and successful retry.

Selection-boundary handles
--------------------------
The boundary transform dialog exposes an on-canvas box. Drag inside to move,
corners to scale about the center, or the blue control to rotate. Keep proportions
locks corner scaling; Shift constrains movement to one axis and snaps rotation to
15-degree steps. Numeric controls and handles share coalesced cancellable mask
previews. Apply records one selection-only undo step; image pixels and the saved
image revision are unchanged. Escape restores the original boundary.

Masked cropping preserves layer properties and original selected RGBA while
clearing excluded pixels to transparent black. Full-canvas ellipses still remove
their outside corners. Undo restores all layer pixels, dimensions and selection.

High-quality image resampling
-----------------------------
Resize image defaults to Catmull-Rom bicubic. Other choices are nearest neighbor,
low-quality bilinear, scale-aware bilinear, smooth cubic B-spline, Lanczos 3,
area reduction with bilinear enlargement, and adaptive Lanczos reduction/bicubic
enlargement. Gamma correction uses the sRGB transfer curve; filtering always
premultiplies alpha. Nearest and identity operations preserve exact channel bytes.
Two separable passes share a bounded 32 MiB filtered-row cache and select axis
order to reduce intermediate work. Cancellation is checked during row generation
and accumulation. Every layer participates in one undoable geometry change.
Exact external kernel and edge-policy equivalence remains an acceptance task.

Reproducible benchmark:
    cargo run --locked --release --example resample_performance -- 2048
On the current Linux host, 2048-square to 1024-square gamma-corrected resizing took
about 120 ms with bicubic and 144 ms with Lanczos. These are CPU kernel timings,
not UI latency guarantees. The example also measures cancellation after 20 ms.

Physical-resolution project metadata
------------------------------------
Document resolution uses standard EXIF XResolution/YResolution rationals and
ResolutionUnit inside project string metadata. Defaults are 96 DPI; valid values
range from 1 to 10000 DPI. Centimeter units are converted to dots per inch.
PRUST, PDN and recovery snapshots preserve these tags and unrelated metadata.
Metadata participates in image dirty state, undo/redo and cancellation, including
disk-backed history. A synthetic 300 DPI PDN fixture was displayed as 1 x 0.5 inches
in the native Windows editor and re-saved without changing its resolution or pixels.
PNG, JPEG, BMP, WebP and TIFF imports/exports preserve independent X/Y DPI.
PNG/BMP use integer pixels per meter (rounding error at most 0.013 DPI); JPEG
and WebP use minimal resolution-only EXIF, and TIFF uses rational density tags.
Orientation changes swap density axes when needed. Source descriptive tags are
not copied. DDS/GIF/TGA do not retain DPI; the export dialog explains this.

Physical units in the workspace
-------------------------------
View > Measurement units selects pixels, inches or centimeters for rulers and
new resize dialogs. Preferences retain the unit and remain compatible with older
settings files. Rulers use the active document's X/Y DPI independently and retain
fractional labels at high zoom. Resize image and Canvas size accept physical
dimensions with visible pixel dimensions, independent or linked X/Y DPI, and
aspect-ratio control. DPI-only changes keep pixels and selection intact; combined
DPI/geometry changes publish one undo step. The resize dialog now defaults to
gamma-corrected bicubic and accepts Enter to apply or Escape to cancel.

Resize image and Canvas size also support percentages relative to the original
pixel dimensions. Canvas expansion can fill each layer's new pixels with
transparency, primary/secondary color, white or black. Existing pixels, hidden
layers and layer properties are preserved; one undo restores the entire edit.
The resize dialog keeps Apply/Cancel outside its scrollable option area.

OpenType text layout
-------------------
Text preview uses Rustybuzz shaping and Unicode bidirectional/script runs before
rasterizing outlines. Combining marks, optional ligatures and contextual forms
use the selected font's tables. Letter spacing applies between shaped clusters;
nonzero spacing disables optional Latin ligatures. Collection face indices are
preserved through font selection, cached reuse, shaping and outline rendering.
Underline and strikeout use font metrics with a fallback for absent metrics.
Alignment, selection clipping, antialiasing, cancellation and one-step undo still
apply. Automatic outline-font fallback is enabled by default and can be disabled
in the text dialog. The preview worker queries Fontconfig only when the selected
font cannot shape a required grapheme. It prefers a single face per script run,
otherwise keeps extended grapheme clusters intact and coalesces adjacent ranges.
Combining sequences are checked after shaping, so composed glyphs do not require
a separate cmap entry for every mark. Supported primary-font text is retained.
The per-preview cache holds at most 32 faces and 64 MiB of font data, checks at
most 128 ordered candidates and reads at most 128 MiB of candidates per request.
The existing 16 MiB per-font limit still applies. Missing Fontconfig or unavailable
outline fonts leave missing glyphs in the selected font. Install the necessary
language fonts when this occurs. Color-font painting, automatic style/axis matching
for fallback faces, and fallback-driven line-height expansion remain pending.
Fallback fonts use their default axes and share the primary font baseline and
em scale, avoiding script-dependent size changes from different ascent metrics.
Unicode format controls and whitespace never paint missing-glyph boxes.
Multilingual fallback probe:
    cargo run --locked --example text_fallback -- /tmp/fallback.png
Typography probe with a locally installed font (the font is not bundled):
    cargo run --locked --example text_shaping -- /path/to/font.ttf /tmp/text.png
Tests cover composed/decomposed accents, bidi override ordering, ligatures,
spacing, font decoration position and invalid collection indices. Arabic and
Hebrew system-font renders have also been inspected on the development host.

Variable fonts expose up to sixteen non-hidden axes in the text dialog, using
localized names supplied by the font, declared ranges and default values.
Reset restores the face defaults; selecting another face clears old coordinates.
Coordinates are validated and applied to both OpenType shaping and outline
rasterization, including variable advances and decoration metrics. Unsupported
axes and non-finite/out-of-range coordinates reject a preview without publication.
A reproducible local-font probe compares regular/bold coverage and writes a PNG:
    cargo run --locked --example font_variations -- /path/to/variable.ttf /tmp/weights.png
The development host's Noto Sans weight range 400..700 produced distinct regular
and bold outlines, with ink coverage increasing from 1363775 to 2186539 units.

Painting blend modes
--------------------
Lines and shapes expose Blend color and fourteen paint modes. Replacement remains
available when Blend color is disabled; outline/fill coverage is combined before
compositing once against the original layer. Color gradients use the chosen mode
only for Blend colors; Replace and Transparency retain their separate behavior.
Text has its own paint-mode selector, including glyph and decoration coverage.
All modes are compared against layer compositing with partially transparent
source/destination colors. Existing preview cancellation and undo still apply.
Text Apply/Cancel remain outside the scrollable controls on compact windows.

Pencil and flood fill share the paint-mode controls. Pencil remains a one-pixel
path and blends against the original stroke snapshot, so repeated samples in one
stroke do not accumulate alpha. Flood fill finds its region using active/merged
sampling, tolerance and selection first, then blends only the matching active-layer
pixels. Turning Blend color off retains exact replacement. Tests compare all
fourteen modes to layer compositing and verify cancellation/undo and selection.

Legacy compressed PDN input
---------------------------
The reader also accepts whole-graph gzip, with or without a PDN3 XML header.
Inline pointerData byte arrays and chunkCount/chunkFormat pixel arrays are read
without constructing serialized objects. Byte arrays have compact storage and a
separate 256 MiB aggregate limit; metadata retains its 16 MiB parser limit.
Gzip checksums, complete buffer lengths, cancellation and collection limits are
validated. Parent-buffer views share bounded backing storage and reject cycles,
excessive nesting and out-of-range offsets. Those views have synthetic reader
tests only: the native Windows editor rejected the generated parent fixture,
so external parent-layout interoperability remains unverified.
Native Windows 5.1.12 opened and re-saved the inline and chunked legacy fixtures
on 2026-10-25 with exact layer pixels/properties, including hidden RGB and alpha.
Normal Save continues to emit the modern deferred-block PDN3 container.

Color selection and sampled fill expose Tolerance alpha. Premultiplied alpha
(the default) compares alpha-scaled RGB plus alpha, treating hidden colors in
fully transparent pixels as equal. Straight alpha compares the stored RGBA
channels, so different hidden RGB can form separate regions. Connectivity,
selection boundaries, merged sampling, cancellation and paint blending still apply.

Recolor can sample the active-layer pixel at the first click of each stroke.
That source color stays fixed while dragging, and the next stroke samples again.
Left button replaces with primary; right button replaces with secondary. The
existing primary/secondary source mode remains available. Selection, tolerance,
coverage and one-step undo apply in both modes. External tolerance-range and soft-edge parity remain separate acceptance work.

Recolor full-coverage reference
------------------------------
The original 350x50 ramp fixture spans seven colors and five alpha levels,
including hidden RGB. With a 1000 px brush, 100% hardness/tolerance, sampling once
at (175,24) and opaque black as replacement, the native Windows 5.1.12 output
matches all 17500 pixels exactly. This validates RGBA shade shifts, clipping and
byte-quantized source-over composition. It does not establish all tolerance
thresholds, antialiased region boundaries or soft brush coverage.
Reproduce the source with examples/recolor_probe.rs; source/output fixtures and
provenance are stored under tests/fixtures/pdn.

Brush dab spacing
-----------------
Brush, Eraser, Clone and Recolor provide a saved Dab spacing percentage. Zero
keeps continuous coverage; positive values place round dabs at intervals relative
to the brush diameter (minimum 0.25 image pixels). Sampling follows path length
across pointer events and bends, so event frequency does not change the spacing.
Off-canvas dabs are skipped before rasterization. Every stroke remains one undo
transaction; overlapping dabs retain the stroke's maximum coverage. Path smoothing
and tablet pressure remain pending.

GIF dithering
-------------
GIF export offers optional Floyd–Steinberg color dithering. The decoded preview
and byte count update when the option changes. Dithering uses alternating row
directions and two bounded error rows, preserves the binary transparency mask,
and leaves exact palette colors unchanged. The transparency threshold remains
128; retained translucent pixels are composited onto the chosen matte. Output
is still a single frame with at most 256 colors.

DDS mipmaps
-----------
DDS export can include every mip level down to 1x1. The base level preserves
all RGBA bytes exactly; subsequent levels use area filtering in linear light
with premultiplied-alpha averaging. Odd dimensions shrink by floor division,
with each dimension clamped to one. Levels are generated sequentially, retaining
only the current source and next level, and generation observes cancellation.
The decoded preview shows the base level; its byte count includes all mipmaps.
Import still opens the top surface. Compressed export and arrays/cube maps remain
pending. Header flags/layout follow the published DDS specification:
https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header
https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-file-layout-for-textures

JPEG XL import
--------------
Open and Import as Layer accept .jxl files, raw JPEG XL codestreams and JPEG XL
containers. The first displayed frame is decoded in Rust using jxl-oxide, with
Little CMS for ICC conversion to sRGB. Codestream orientation is applied once.
RGB, grayscale and straight alpha are converted to the editor's RGBA8 workspace;
16-bit SDR inputs are quantized after color conversion. Source descriptive
metadata is omitted. Physical resolution is read from EXIF density metadata;
files without it default to 96 DPI. Still-image trailing metadata is supported;
animation import reads metadata available with the first displayed frame.

Limits: 16,384 pixels per side, the document pixel limit, 256 MiB of compressed
input consumed for a still image or the first animation frame, and a 256 MiB
decoder grid allocation budget.
The allocation tracker does not cap all metadata or process memory. Cancellation
is checked during input and output conversion, and before/after frame rendering;
the decoder's internal render call is not interruptible. HDR tone mapping and
associated-alpha input remain unsupported and produce explicit errors.

JPEG XL lossless RGBA8 export is available through Save As and Export. It declares
sRGB in its codestream and supports one-pixel dimensions. The preview decodes the
exact bytes saved; a minimal EXIF box stores independent X/Y DPI in the JPEG XL
container. Source descriptive metadata is omitted. The encoder runs in a background worker; cancellation is checked
before and after its internal encode call. Lossy encoding remains pending.
The Linux desktop entry includes image/jxl. Decoder documentation:
https://docs.rs/jxl-oxide/0.12.6/jxl_oxide/

AVIF still images
-----------------
Open, Import as Layer, Save As and Export support AVIF still images. Import uses
dav1d through the image adapter; export uses the Rust ravif/rav1e encoder. The
quality slider affects both color and alpha. Quality 100 is not a promise of
lossless output. Preview decodes the exact bytes that will be saved.

The importer supports RGB/grayscale ICC conversion to sRGB, container rotation
and mirroring, and straight alpha. Untagged images are treated as sRGB. HDR,
associated alpha, animation, grids, clean-aperture cropping, layered items,
non-square pixels, conflicting container color matrices/ranges, and unsupported
non-sRGB color declarations without ICC are rejected explicitly. Property checks
are conservative and can also reject unsupported properties of unused items.
Source descriptive metadata is omitted. This first AVIF implementation does not
retain physical resolution: reopening uses 96 DPI. It does not provide HEIC.

Encoded input is limited to 128 MiB, ICC profiles to 4 MiB, and images to
16,777,216 pixels with a maximum side of 16,384 pixels. Each coded item is drained
with dav1d's native frame-size limit before the image adapter allocates its frames;
multiple coded frames per item are rejected. This entails a second decoding pass.
These are input/frame limits, not a strict cap on total process memory. Cancellation
is checked around native calls, reads and pixel rows; a codec call itself may finish
before cancellation takes effect. Export uses two encoder threads.

Building now requires dav1d development headers and pkg-config (for example,
libdav1d-devel on Fedora or libdav1d-dev on Debian-family systems). The executable
also requires the matching system dav1d runtime; the installer does not bundle it.
Independent libaom fixtures cover pixels, alpha, color profiles and rejected image
sizes. Synthetic associated-property fixtures cover rotation and mirroring.
