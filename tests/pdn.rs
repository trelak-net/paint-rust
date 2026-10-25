use paint_rust::pdn;
use std::sync::atomic::AtomicBool;

#[test]
fn reads_independent_legacy_and_modern_documents() {
    for name in [
        "Untitled2.pdn",
        "oldPDN3510.pdn",
        "FlattenBlendTest.pdn",
        "Native5112Resaved.pdn",
    ] {
        let path = std::path::Path::new("tests/fixtures/pdn").join(name);
        let file = std::fs::File::open(path).unwrap();
        let result = pdn::read(std::io::BufReader::new(file), &AtomicBool::new(false));
        let data = result.unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(data.document.width > 0);
        assert_eq!(
            data.document.layers().len(),
            if name == "FlattenBlendTest.pdn" {
                14
            } else {
                2
            }
        );
    }
}

#[test]
fn external_resave_preserves_layer_pixels() {
    let read = |name| {
        let f = std::fs::File::open(std::path::Path::new("tests/fixtures/pdn").join(name)).unwrap();
        pdn::read(std::io::BufReader::new(f), &AtomicBool::new(false)).unwrap()
    };
    let a = read("Untitled2.pdn");
    let b = read("Native5112Resaved.pdn");
    assert!(a.document.layers() == b.document.layers());
}

#[test]
fn roundtrip_preserves_all_layers_properties_and_metadata() {
    for name in ["Untitled2.pdn", "oldPDN3510.pdn", "FlattenBlendTest.pdn"] {
        let file =
            std::fs::File::open(std::path::Path::new("tests/fixtures/pdn").join(name)).unwrap();
        let a = pdn::read(std::io::BufReader::new(file), &AtomicBool::new(false)).unwrap();
        let mut encoded = Vec::new();
        pdn::write(&a, &mut encoded, &AtomicBool::new(false)).unwrap();
        let b = pdn::read(encoded.as_slice(), &AtomicBool::new(false)).unwrap();
        assert!(a.document.layers() == b.document.layers());
        assert_eq!(a.metadata, b.metadata);
        assert_eq!(a.layer_metadata, b.layer_metadata);
        assert_eq!(a.backgrounds, b.backgrounds);
    }
}

#[test]
fn rejects_truncation_trailing_data_and_cancelled_io() {
    let input = include_bytes!("fixtures/pdn/Untitled2.pdn");
    for end in [0, 4, 7, 100, input.len() / 2, input.len() - 1] {
        assert!(pdn::read(&input[..end], &AtomicBool::new(false)).is_err());
    }
    let mut trailing = input.to_vec();
    trailing.push(0);
    assert!(pdn::read(trailing.as_slice(), &AtomicBool::new(false)).is_err());
    assert!(pdn::read(input.as_slice(), &AtomicBool::new(true)).is_err());
    let data = pdn::read(input.as_slice(), &AtomicBool::new(false)).unwrap();
    assert!(pdn::write(&data, Vec::new(), &AtomicBool::new(true)).is_err());
}

#[test]
fn editor_project_and_snapshot_preserve_interchange_metadata() {
    let input = std::path::Path::new("tests/fixtures/pdn/Untitled2.pdn");
    let original = paint_rust::files::open(input).unwrap();
    assert!(!original.metadata.is_empty());
    let directory = tempfile::tempdir().unwrap();
    let project = directory.path().join("metadata.prust");
    paint_rust::files::save_project(&original.snapshot(), &project).unwrap();
    let mut recovered = paint_rust::files::open(&project).unwrap();
    recovered.add_layer(true);
    recovered.undo();
    let output = directory.path().join("metadata.pdn");
    paint_rust::files::save_project(&recovered, &output).unwrap();
    let reopened = paint_rust::files::open(&output).unwrap();
    assert_eq!(original.metadata, reopened.metadata);
    assert!(original.layers() == reopened.layers());
}

#[test]
fn windows_gui_resave_of_rust_output_preserves_every_layer_pixel_and_property() {
    let original =
        paint_rust::files::open(std::path::Path::new("tests/fixtures/pdn/Untitled2.pdn")).unwrap();
    let native = paint_rust::files::open(std::path::Path::new(
        "tests/fixtures/pdn/RustNativeResaved.pdn",
    ))
    .unwrap();
    assert_eq!(
        (original.width, original.height),
        (native.width, native.height)
    );
    assert!(original.layers() == native.layers());
    // The external writer may update application/version EXIF strings.
    assert_eq!(original.metadata.len(), native.metadata.len());
}

#[test]
fn windows_gui_preserves_fourteen_blend_modes_from_rust_output() {
    let original = paint_rust::files::open(std::path::Path::new(
        "tests/fixtures/pdn/FlattenBlendTest.pdn",
    ))
    .unwrap();
    let native = paint_rust::files::open(std::path::Path::new(
        "tests/fixtures/pdn/RustAllBlendsResaved.pdn",
    ))
    .unwrap();
    assert_eq!(native.layers().len(), 14);
    assert!(original.layers() == native.layers());
}

#[test]
fn merge_down_matches_external_result_and_retains_lower_properties() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdn");
    for (input, output) in [
        ("MergeInput.pdn", "MergeNativeOutput.pdn"),
        ("MergeAllModesInput.pdn", "MergeAllModesNativeOutput.pdn"),
    ] {
        let mut expected = paint_rust::files::open(&root.join(input)).unwrap();
        for top in (1..expected.layers().len()).step_by(2).rev() {
            expected.select_layer(top);
            assert_eq!(
                expected.merge_down_cancellable(&std::sync::atomic::AtomicBool::new(false)),
                Some(true)
            );
        }
        let actual = paint_rust::files::open(&root.join(output)).unwrap();
        assert_eq!(actual.layers().len(), expected.layers().len());
        for (a, b) in actual.layers().iter().zip(expected.layers()) {
            assert_eq!(a.pixels(), b.pixels(), "{}", a.name);
            assert_eq!(
                (
                    &a.name,
                    a.visible,
                    a.opacity,
                    a.blend_mode,
                    a.is_background,
                    &a.metadata
                ),
                (
                    &b.name,
                    b.visible,
                    b.opacity,
                    b.blend_mode,
                    b.is_background,
                    &b.metadata
                )
            );
        }
    }
}

#[test]
fn external_resave_preserves_unicode_metadata_hidden_rgb_and_opacity_edges() {
    let input =
        paint_rust::files::open(std::path::Path::new("tests/fixtures/pdn/MetadataInput.pdn"))
            .unwrap();
    let native = paint_rust::files::open(std::path::Path::new(
        "tests/fixtures/pdn/MetadataNativeResaved.pdn",
    ))
    .unwrap();
    assert_eq!((native.width, native.height), (17, 13));
    assert_eq!(native.layers().len(), 5);
    for (index, (a, b)) in input.layers().iter().zip(native.layers()).enumerate() {
        assert_eq!(a.name, b.name, "layer {index} name");
        assert_eq!(a.pixels(), b.pixels(), "layer {index} pixels");
        assert_eq!(a.visible, b.visible, "layer {index} visibility");
        assert_eq!(a.opacity, b.opacity, "layer {index} opacity");
        assert_eq!(a.blend_mode, b.blend_mode, "layer {index} blend");
        assert_eq!(a.is_background, b.is_background, "layer {index} background");
        assert_eq!(a.metadata, b.metadata, "layer {index} metadata");
    }
    for (key, value) in &input.metadata {
        assert_eq!(native.metadata.get(key), Some(value));
    }
}

#[test]
fn external_layer_rename_preserves_other_properties_and_metadata() {
    let original =
        paint_rust::files::open(std::path::Path::new("tests/fixtures/pdn/MetadataInput.pdn"))
            .unwrap();
    let edited = paint_rust::files::open(std::path::Path::new(
        "tests/fixtures/pdn/MetadataNativeEdited.pdn",
    ))
    .unwrap();
    assert_eq!(edited.layers().len(), 5);
    assert_eq!(edited.layers()[4].name, "Edited top layer");
    for (index, (a, b)) in original.layers().iter().zip(edited.layers()).enumerate() {
        let mut expected = a.clone();
        if index == 4 {
            expected.name = "Edited top layer".into();
        }
        assert!(expected == *b, "layer {index} changed beyond its name");
    }
    for (key, value) in &original.metadata {
        assert_eq!(edited.metadata.get(key), Some(value));
    }
    let directory = tempfile::tempdir().unwrap();
    let saved = directory.path().join("edited.pdn");
    paint_rust::files::save_project(&edited, &saved).unwrap();
    let reopened = paint_rust::files::open(&saved).unwrap();
    assert!(edited.layers() == reopened.layers());
    assert_eq!(edited.metadata, reopened.metadata);
}
