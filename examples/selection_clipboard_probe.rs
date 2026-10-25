use paint_rust::{
    document::{Document, Selection},
    selection::{self, CombineMode, clipboard},
};
use std::sync::atomic::AtomicBool;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let cancel = AtomicBool::new(false);
    let outer = Selection::elliptical(1, 1, 15, 11);
    let hole = Selection::elliptical(5, 4, 7, 5);
    let selection =
        selection::combine_cancellable(17, 13, Some(&outer), &hole, CombineMode::Subtract, &cancel)
            .unwrap();
    let encoded = clipboard::encode(17, 13, Some(&selection), &cancel).unwrap();
    std::fs::write(&args[0], encoded).unwrap();
    if let Some(path) = args.get(1) {
        let text = std::fs::read_to_string(path).unwrap();
        let native =
            clipboard::decode(17, 13, text.trim_start_matches('\u{feff}'), &cancel).unwrap();
        let raster = |selection| {
            let mut document = Document::new(17, 13, [0, 0, 0, 255]).unwrap();
            document.set_selection(Some(selection));
            document.invert();
            document.pixels().to_vec()
        };
        assert_eq!(raster(selection), raster(native));
        println!("Selection pixels match after external clipboard round-trip");
    }
}
