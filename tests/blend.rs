use paint_rust::{
    document::{BlendMode, Document},
    files,
};

fn layered() -> Document {
    let mut doc = Document::new(1, 1, [100, 150, 200, 255]).unwrap();
    doc.add_layer(false);
    doc.begin("Paint");
    doc.fill(0, 0, [200, 100, 50, 255]);
    doc.commit();
    doc
}

#[test]
fn blend_modes_match_opaque_reference_values_and_undo() {
    let mut doc = layered();
    for (mode, expected) in [
        (BlendMode::Multiply, [78, 59, 39, 255]),
        (BlendMode::Screen, [222, 191, 211, 255]),
        (BlendMode::Darken, [100, 100, 50, 255]),
        (BlendMode::Lighten, [200, 150, 200, 255]),
        (BlendMode::Difference, [100, 50, 150, 255]),
        (BlendMode::Add, [255, 250, 250, 255]),
    ] {
        doc.set_layer_style("Top".into(), 255, mode);
        assert_eq!(doc.composite()[0], expected);
    }
    doc.undo();
    assert_eq!(doc.layers()[1].blend_mode, BlendMode::Difference);
    doc.redo();
    assert_eq!(doc.layers()[1].blend_mode, BlendMode::Add);
}

#[test]
fn blend_modes_do_not_invent_a_backdrop_for_transparent_pixels() {
    let mut doc = Document::new(1, 1, [123, 45, 67, 0]).unwrap();
    doc.add_layer(false);
    doc.begin("Paint");
    doc.fill(0, 0, [100, 150, 200, 128]);
    doc.commit();
    for mode in BlendMode::ALL {
        doc.set_layer_style("Top".into(), 255, mode);
        assert_eq!(doc.composite()[0], [100, 150, 200, 128]);
        doc.set_layer_style("Top".into(), 0, mode);
        assert_eq!(doc.composite()[0], [123, 45, 67, 0]);
    }
}

#[test]
fn project_v2_preserves_modes_and_v1_remains_readable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("blend.prust");
    let mut doc = layered();
    for mode in BlendMode::ALL {
        doc.set_layer_style("Top".into(), 123, mode);
        files::save_project(&doc, &path).unwrap();
        let loaded = files::open(&path).unwrap();
        assert_eq!(loaded.layers()[1].blend_mode, mode);
        assert_eq!(loaded.composite(), doc.composite());
    }
    let single = Document::new(2, 2, [255; 4]).unwrap();
    files::save_project(&single, &path).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    let name_length = u32::from_le_bytes(bytes[24..28].try_into().unwrap()) as usize;
    let mode_index = 28 + name_length + 2;
    let mut invalid = bytes.clone();
    invalid[mode_index] = 255;
    std::fs::write(&path, invalid).unwrap();
    assert!(files::open(&path).is_err());
    bytes[6] = 1;
    bytes.remove(mode_index);
    std::fs::write(&path, bytes).unwrap();
    let loaded = files::open(&path).unwrap();
    assert_eq!(loaded.layers()[0].blend_mode, BlendMode::Normal);
    assert_eq!(loaded.pixels(), single.pixels());
}
