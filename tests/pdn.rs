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
