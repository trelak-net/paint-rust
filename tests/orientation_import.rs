use image::{ImageEncoder, codecs::jpeg::JpegEncoder};
use paint_rust::files;

fn jpeg(orientation: u16) -> Vec<u8> {
    let mut exif = b"II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    exif.extend(orientation.to_le_bytes());
    exif.extend([0; 6]);
    let mut encoded = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut encoded, 100);
    encoder.set_exif_metadata(exif).unwrap();
    encoder
        .encode(
            &[
                255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0, 255, 0, 255, 0, 255, 255,
            ],
            3,
            2,
            image::ExtendedColorType::Rgb8,
        )
        .unwrap();
    encoded
}

#[test]
fn all_exif_orientations_are_applied_once_with_clean_history() {
    let directory = tempfile::tempdir().unwrap();
    let raw = image::load_from_memory(&jpeg(1)).unwrap().to_rgba8();
    let original: Vec<_> = raw.pixels().map(|p| p.0).collect();
    let orders = [
        [0, 1, 2, 3, 4, 5],
        [2, 1, 0, 5, 4, 3],
        [5, 4, 3, 2, 1, 0],
        [3, 4, 5, 0, 1, 2],
        [0, 3, 1, 4, 2, 5],
        [3, 0, 4, 1, 5, 2],
        [5, 2, 4, 1, 3, 0],
        [2, 5, 1, 4, 0, 3],
    ];
    for orientation in 1..=8 {
        let path = directory
            .path()
            .join(format!("orientation-{orientation}.jpg"));
        std::fs::write(&path, jpeg(orientation)).unwrap();
        let document = files::open(&path).unwrap();
        assert_eq!(
            (document.width, document.height),
            if orientation >= 5 { (2, 3) } else { (3, 2) }
        );
        let expected: Vec<_> = orders[orientation as usize - 1]
            .iter()
            .map(|i| original[*i])
            .collect();
        assert_eq!(document.pixels(), expected, "orientation {orientation}");
        assert!(!document.dirty());
        assert!(!document.can_undo());
        let project = directory.path().join("normalized.prust");
        let png = directory.path().join("normalized.png");
        files::save_project(&document, &project).unwrap();
        files::save_png(&document, &png).unwrap();
        for path in [project, png] {
            let reopened = files::open(&path).unwrap();
            assert_eq!(
                (reopened.width, reopened.height),
                (document.width, document.height)
            );
            assert_eq!(reopened.pixels(), document.pixels());
        }
    }
}

#[test]
fn invalid_orientation_is_ignored_and_cancelled_import_stays_cancelled() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("invalid-orientation.jpg");
    std::fs::write(&path, jpeg(99)).unwrap();
    let document = files::open(&path).unwrap();
    assert_eq!((document.width, document.height), (3, 2));
    assert_eq!(
        files::open_cancellable(&path, &std::sync::atomic::AtomicBool::new(true))
            .err()
            .unwrap(),
        files::CANCELLED
    );
}
