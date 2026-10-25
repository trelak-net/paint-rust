use paint_rust::{
    document::Document,
    files,
    resolution::{self, Dpi},
};
use std::sync::atomic::AtomicBool;
#[test]
fn resolution_metadata_round_trips_through_projects_and_pdn() {
    let dir = tempfile::tempdir().unwrap();
    let mut doc = Document::new(17, 13, [12, 34, 56, 128]).unwrap();
    doc.metadata
        .insert("comment".into(), "Preserve unrelated metadata".into());
    let dpi = Dpi {
        x: 300.125,
        y: 150.5,
    };
    assert!(doc.set_resolution(dpi).unwrap());
    assert_eq!(doc.resolution(), dpi);
    for extension in ["prust", "pdn"] {
        let path = dir.path().join(format!("resolution.{extension}"));
        files::save_project(&doc, &path).unwrap();
        let loaded = files::open(&path).unwrap();
        assert_eq!(loaded.resolution(), dpi);
        assert_eq!(loaded.metadata, doc.metadata);
        assert_eq!(loaded.pixels(), doc.pixels());
    }
}
#[test]
fn resolution_changes_are_dirty_undoable_and_survive_disk_history() {
    let mut doc = Document::new(2, 2, [255; 4]).unwrap();
    let dpi = Dpi { x: 300.0, y: 600.0 };
    let original = doc.metadata.clone();
    doc.set_resolution(dpi).unwrap();
    assert!(doc.dirty());
    assert_eq!(doc.history().count(), 1);
    assert!(!doc.set_resolution(dpi).unwrap());
    assert!(doc.store_history_with_limits_cancellable(0, 100000, &AtomicBool::new(false)));
    assert!(doc.undo());
    assert_eq!(doc.metadata, original);
    assert_eq!(doc.resolution(), Dpi::default());
    assert!(!doc.dirty());
    assert!(doc.redo());
    assert_eq!(doc.resolution(), dpi);
    assert_eq!(doc.pixels(), &[[255; 4]; 4]);
    let metadata = doc.metadata.clone();
    doc.begin("Cancelled metadata edit");
    doc.metadata.clear();
    doc.cancel();
    assert_eq!(doc.metadata, metadata);
}
#[test]
fn native_metadata_fixture_has_standard_96_dpi_and_invalid_values_fall_back() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/pdn/MetadataNativeResaved.pdn");
    let doc = files::open(&path).unwrap();
    assert_eq!(doc.resolution(), Dpi { x: 96.0, y: 96.0 });
    assert!(doc.metadata.values().any(|v| v.contains("id=\"282\"")));
    let mut data = doc.metadata.clone();
    for value in data.values_mut() {
        if value.contains("id=\"282\"") {
            *value = "<exif id=\"282\" len=\"8\" type=\"5\" value=\"not-base64\" />".into();
        }
    }
    assert_eq!(resolution::read(&data), Dpi::default());
    let original = data.clone();
    assert!(
        resolution::write(
            &mut data,
            Dpi {
                x: f64::NAN,
                y: 300.0
            }
        )
        .is_err()
    );
    assert_eq!(data, original);
}
#[test]
fn centimeters_are_converted_to_inches() {
    let mut metadata = std::collections::BTreeMap::new();
    resolution::write(&mut metadata, Dpi { x: 100.0, y: 200.0 }).unwrap();
    for value in metadata.values_mut() {
        if value.contains("id=\"296\"") {
            *value = "<exif id=\"296\" len=\"2\" type=\"3\" value=\"AwA=\" />".into();
        }
    }
    assert_eq!(resolution::read(&metadata), Dpi { x: 254.0, y: 508.0 });
}

#[test]
fn external_editor_preserves_generated_resolution_and_pixels() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pdn");
    let input = files::open(&root.join("ResolutionInput.pdn")).unwrap();
    let output = files::open(&root.join("ResolutionNativeResaved.pdn")).unwrap();
    assert_eq!(input.resolution(), Dpi { x: 300.0, y: 300.0 });
    assert_eq!(output.resolution(), input.resolution());
    assert_eq!((output.width, output.height), (300, 150));
    assert_eq!(output.pixels(), input.pixels());
}
