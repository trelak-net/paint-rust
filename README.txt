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
- Sharpen local RGB contrast while preserving alpha and the active selection.
- Draw lines and outlined/filled rectangles or ellipses with cancellable previews.
- Select a rectangular or elliptical area, limit edits to it, delete pixels or crop all layers.
- Add, subtract or intersect selections; combined areas use a blue mask overlay.
- Build polygon selections by clicking vertices; Enter/double-click finishes them.
- Resize all layers, change canvas size with a nine-position anchor, rotate or mirror.
- Free-transform selected pixels with rotation, scaling, mirroring and offsets.
- Hold Shift for 45-degree line angles or equal shape dimensions.
- Zoom from 10% to 400%, fit the canvas, and navigate with scrollbars.
- Use Ctrl+wheel or a pinch gesture to zoom around the pointer; middle-drag to pan.
- Undo/redo complete strokes and edits; cancel a stroke with Escape.
- Expand History in the layer panel to undo or redo multiple operations at once.
- Open PNG, JPEG, BMP and WebP; export flattened RGBA PNG or lossless WebP.
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
P: polygon selection         Enter: finish polygon; Backspace: remove last point
G: gradient                  X: swap primary/secondary colors
Move tool: arrows move 1 px; Shift+arrows move 10 px; Ctrl+arrows copy pixels
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
shares unchanged layer buffers and keeps up to 128 MiB of unique pixel and
selection-mask buffers in RAM across undo/redo. Older states spill into private
temporary files instead of being evicted. Labels and metadata remain in RAM. The
active document is limited to 256 MiB of layer pixels, excluding previews/history.
A canvas is limited to 16,777,216 pixels. The New dialog permits up to 4096 per axis.

Save writes an editable .prust project. Raster export writes only the visible composite
and does not mark an edited project as saved. Project version 2 stores blend modes as well as dimensions,
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
this is not a guarantee of constant frame time at the document limits. Metadata, orientation tags and embedded color profiles are not
preserved or interpreted. PNG and lossless WebP are available for raster export. A user-local Linux installation helper is available; there is no signed
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
clipped movement preserve mask geometry. Crop still uses rectangular bounds.

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
Branching after undo releases obsolete redo files. Disk usage is not capped;
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
masked selections crop to their rectangular bounds.
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
Whole-graph gzip, parent-backed buffers, non-bitmap layers and future signatures
are not supported yet. ICC interpretation and metadata updates after geometry
changes remain separate work. Reproduce a codec round trip with:

    cargo run --example pdn_roundtrip -- INPUT OUTPUT.pdn
