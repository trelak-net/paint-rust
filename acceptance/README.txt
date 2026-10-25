Desktop workflow acceptance inventory
=====================================
This inventory is a working contract for the full implementation program.
It supplements ROADMAP.txt; a completed implementation item is not a claim of
complete numerical, visual or interaction parity.

Status meanings
---------------
supported: the command is available and its core workflow has automated coverage.
partial: a usable subset exists, but options, shortcuts or output still differ.
missing: the feature has not been implemented.
different: an intentional product decision, additional format or excluded service.

Scope
-----
commands.tsv lists 79 desktop command capabilities.
tools.tsv lists 19 editing/navigation capabilities, including separate selection
and pixel movement even where the current interface shares a tool.
effects.tsv lists 54 built-in adjustment/filter/transform capabilities. A similar
filter is marked partial, never silently treated as the same algorithm.
formats.tsv lists 14 target or additional document/raster formats.
options.tsv lists 29 tool parameter groups. dialogs.tsv records 17 required or
additional dialog flows. Command/tool tables show expected and current shortcuts;
a dash means no explicit binding is currently recorded. Shared expected tool keys
require cycling, which is still missing from the current direct-key mapping.

PDN is required. External managed plugin compatibility is deferred. Project
identity, original icons, the PRUST format and project-specific help destinations
are intentional differences. Unsupported formats must not be advertised as usable.
Full accessibility acceptance remains the final stage of the program.

Repeatable input sets
---------------------
Use transparent, opaque and mixed-alpha images; include hidden RGB at alpha zero.
Use two layers for basic operations and fourteen modes for blend interoperability.
Use square, wide, tall, one-pixel, empty-selection and disjoint-selection cases.
For UI review use both 800x600 and 1920x1080 logical workspaces; repeat at 100%,
150% and 200% desktop scaling in light and dark themes. Native file dialogs are
reviewed on the host desktop. Do not compare screenshot pixels across different
font, scaling or theme settings as if they were algorithmic output differences.

Acceptance cases and evidence
-----------------------------
DOC-01: Open two files, edit each, switch tabs, undo independently, reorder tabs.
Expected: image, active layer, selection, path, history and view remain independent.
Evidence: workspace unit tests. Global resident-memory budgeting remains pending.
DOC-02: Close several dirty documents. Exercise Save, Discard and Cancel at each
step, including a failed save. Expected: no unapproved document loss.
Evidence: workspace close/recovery tests; disk-full process-level checks pending.
DOC-03: Drop several valid and invalid files together. Expected: valid documents
open in tabs and failures are collected without replacing prior work.
Evidence: queued-open workspace tests.

PDN-01: Open and re-save layered codec output in an external desktop editor.
Expected: decoded layer pixels, names, opacity, visibility, blend modes and string
metadata survive. Evidence: RustNativeResaved and RustAllBlendsResaved fixtures,
tests/pdn.rs and tests/fixtures/pdn/SOURCES.txt. Broader PDN variants are pending.
PDN-02: Merge adjacent pairs with hidden layers, partial alpha and opacity.
Expected: exact saved layer pixels/properties for all fourteen modes.
Evidence: MergeAllModesInput/MergeAllModesNativeOutput fixtures and tests/pdn.rs.

LAY-01: Drag any layer onto any other layer with each possible active-layer index.
Expected: active identity is retained and one Undo restores the complete order.
Evidence: tests/layers.rs. Thumbnail cache tests check pixel invalidation and
that old full-size buffers are not retained by thumbnails.
LAY-02: Merge, flatten and undo with visibility, metadata and blend settings.
Expected: one history entry and exact restoration of the original layers.
Evidence: tests/layers.rs and the external PDN merge cases.

SEL-01: Combine overlapping rectangles, ellipses, polygons and lasso outlines
using replace/add/subtract/intersect/exclusive-or; invert empty and full masks.
Expected: correct pixel membership, with empty distinct from no selection.
Evidence: selection test suites and editor gesture tests.
SEL-02: Select and fill adjacent, diagonal and disconnected colors. Change
tolerance and active/merged sampling. Include a selection with disconnected parts.
Expected: four-neighbor connectivity, fixed seed comparison and selection barriers.
Evidence: tests/color_selection.rs, tests/fill_options.rs. Exact tolerance matching
and antialiased boundaries still require external output comparisons.
NAV-01: Pan with the tool, Space and middle button; release Space before the mouse.
Zoom around the pointer and fit a dragged region. Expected: no pixel/history edits.
Evidence: editor navigation tests. Complete rollback and zoom-step parity pending.

TOOL-01: Draw with both mouse buttons. Verify primary/secondary colors, continuous
pencil paths, shape constraints and one-step undo. Cancel previews with Escape.
Evidence: pencil/shapes/editor tests. Brush coverage, pressure and curve tools
remain incomplete; a hard brush is not accepted as full brush parity.
TOOL-02: Draw all five gradient geometries in blend, replace and transparency
modes. Reverse the drag button and use a partial selection.
Evidence: tests/gradient.rs and gesture preview tests. Persistent endpoint editing
and numerical color-space comparisons are pending.
TXT-01: Enter multiple lines, align each line, adjust spacing and edge smoothing.
Load a valid font, then an invalid font, then dismiss the draft during loading.
Expected: late results never recreate a dismissed dialog; Apply is one edit.
Evidence: tests/text.rs and editor font-loading tests. Full shaping/font browser
and decoration controls remain pending.

FX-01: Rapidly change effect parameters; cancel during rendering; apply only the
latest result. Repeat on a different selection and undo once.
Expected: no stale publication or parameter replacement after Cancel.
Evidence: preview/editor/effect tests. Every new filter must join these checks.
FX-02: Test neutral settings, channel endpoints, gamma, transparent outliers,
constant channels and histogram clipping. Expected: alpha/selection preservation.
Evidence: tests/effects.rs and tests/levels.rs. Exact reference working-space
comparisons are still pending for adjustments; these are not pixel-parity claims.

UI-01: Hide/show and reposition panels, restart twice and compare saved layout.
Expected: stable floating positions/content sizes and visibility, readable controls
and usable layer thumbnails. Native smoke script: scripts/ui_layout_smoke.py.
UI-02: Audit menu grouping, tool options, dialogs and shortcuts at every listed
workspace size/theme/scale. Status: pending comprehensive visual review.
UI-03: Full keyboard and native screen-reader workflow. Status: final-stage task.

FILE-01: Reopen each exported format and verify dimensions, alpha and metadata
policy. Compare the decoded preview with the actual saved bytes. Status: partial;
PNG/lossless WebP and project tests exist, export configuration remains pending.
FILE-02: Verify EXIF orientation and ICC conversion exactly once, including
round-trips through project/recovery/PDN. Status: pending.
PERF-01: Benchmark large images and many documents while editing, saving,
recovering and spilling undo to disk. Status: per-document checks exist; shared
memory limits and full multi-document benchmarks remain pending.
FAIL-01: Exercise truncated input, cancelled operations, full disks, lost cache
files, interrupted writes and recovery. Status: automated subsets exist; complete
fault-injection matrix remains pending.

Working rule
------------
Update the matching row and ROADMAP.txt when a feature is implemented. Record
which cases ran and keep unresolved differences visible. A source-level algorithm
inspection alone does not substitute for an external saved-output fixture.
