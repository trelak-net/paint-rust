use super::*;

pub(super) struct ExportDialog {
    save_document: bool,
    flatten_confirmed: bool,
    path: PathBuf,
    options: files::RasterOptions,
    preview: crate::raster_preview::RasterPreview,
    texture: Option<TextureHandle>,
    encoded: Option<std::sync::Arc<Vec<u8>>>,
    error: Option<String>,
    dimensions: (u32, u32),
}
impl Editor {
    pub(super) fn begin_raster_save(&mut self, path: PathBuf, format: files::RasterFormat) {
        self.begin_export(path, format);
        if let Some(dialog) = &mut self.export_dialog {
            dialog.save_document = true;
        }
    }

    pub(super) fn begin_export(&mut self, path: PathBuf, format: files::RasterFormat) {
        self.finish_stroke();
        let options = files::RasterOptions {
            dpi: self.document.resolution(),
            format,
            ..self.export_options
        };
        let preview = crate::raster_preview::RasterPreview::new(
            self.context.clone(),
            self.document.snapshot(),
            options,
        );
        self.export_dialog = Some(ExportDialog {
            save_document: false,
            flatten_confirmed: false,
            path,
            options,
            preview,
            texture: None,
            encoded: None,
            error: None,
            dimensions: (self.document.width, self.document.height),
        });
    }
    pub(super) fn show_export(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.export_dialog.take() else {
            return;
        };
        if let Some(result) = dialog.preview.poll() {
            match result {
                Ok(result) => {
                    if let Some(texture) = &mut dialog.texture {
                        texture.set(result.image, TextureOptions::LINEAR);
                    } else {
                        dialog.texture = Some(ctx.load_texture(
                            "export-preview",
                            result.image,
                            TextureOptions::LINEAR,
                        ));
                    }
                    dialog.encoded = Some(result.bytes);
                    dialog.error = None;
                }
                Err(error) => {
                    dialog.error = Some(error);
                    dialog.encoded = None;
                }
            }
        }
        let mut keep = true;
        let mut save = None;
        let mut open = true;
        egui::Window::new(format!("{} {}",if dialog.save_document {"Save"} else {"Export"},dialog.options.format.label())).collapsible(false).resizable(true).vscroll(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).max_height((ctx.content_rect().height()-80.0).max(180.0)).default_width(540.0).open(&mut open).show(ctx,|ui| {
            egui::ScrollArea::vertical().max_height((ctx.content_rect().height()-220.0).max(100.0)).show(ui, |ui| {
            ui.add(egui::Label::new(dialog.path.display().to_string()).truncate()).on_hover_text(dialog.path.display().to_string());
            if dialog.save_document {
                ui.label("Saving as a raster replaces the current document with the flattened decoded image. You can undo this after saving.");
                ui.small("Layers and descriptive metadata are not stored in this file. Use PRUST or PDN to keep an editable project. Lossy formats change image detail; JPEG cannot store transparency.");
                ui.checkbox(&mut dialog.flatten_confirmed,"Save a flattened raster document");
            }
            ui.small(if dialog.options.format == files::RasterFormat::Avif {
                "AVIF declares sRGB. Source EXIF, location and author tags are omitted."
            } else if dialog.options.format == files::RasterFormat::JpegXl {
                "JPEG XL declares sRGB in the codestream. Source EXIF, location and author tags are omitted."
            } else if matches!(dialog.options.format,files::RasterFormat::Bmp | files::RasterFormat::Tga | files::RasterFormat::Gif | files::RasterFormat::Dds) {
                "Colors use sRGB; this format is saved without an embedded profile."
            } else { "Colors use sRGB; an ICC profile is embedded. Source EXIF, location and author tags are omitted." });
            if matches!(dialog.options.format, files::RasterFormat::Dds | files::RasterFormat::Gif | files::RasterFormat::Tga | files::RasterFormat::Avif) {
                ui.small("Physical resolution is not stored in this format; reopening uses 96 DPI.");
            } else {
                ui.small(format!("Physical resolution: {:.2} × {:.2} DPI.", dialog.options.dpi.x, dialog.options.dpi.y));
            }
            let mut changed=false;
            if matches!(dialog.options.format, files::RasterFormat::Jpeg | files::RasterFormat::Avif) {
                changed |= ui.add(egui::Slider::new(&mut dialog.options.quality,1..=100).text("Quality")).changed();
            }
            if matches!(dialog.options.format,files::RasterFormat::Jpeg | files::RasterFormat::Bmp | files::RasterFormat::Gif) {
                ui.horizontal(|ui| { ui.label("Transparency background"); changed |= ui.color_edit_button_srgb(&mut dialog.options.matte).changed(); });
                ui.small("Transparency is composited onto this color before encoding.");
            }
            if dialog.options.format==files::RasterFormat::Dds {
                egui::ComboBox::from_label("Compression").selected_text(dialog.options.dds_compression.label()).show_ui(ui,|ui| {
                    for compression in paint_rust::dds::Compression::ALL { changed |= ui.selectable_value(&mut dialog.options.dds_compression,compression,compression.label()).changed(); }
                });
                changed |= ui.checkbox(&mut dialog.options.dds_mipmaps, "Generate mipmaps down to 1 × 1").changed();
                ui.small("RGBA8 is lossless. BC1/BC3 use fast lossy block compression; BC1 makes alpha below 128 transparent and other pixels opaque. Inspect the decoded preview. Mipmaps use alpha-aware, linear-light area filtering. The preview shows the full-size image. Texture arrays and cube maps are not written.");
            }
            if dialog.options.format==files::RasterFormat::Gif {
                changed |= ui.checkbox(&mut dialog.options.gif_dither, "Dither colors (Floyd–Steinberg)").changed();
                ui.small("GIF stores one frame and up to 256 colors. Alpha below 128 becomes transparent; remaining pixels use the background color. Inspect the preview for palette changes.");
            }
            if dialog.options.format==files::RasterFormat::Avif {
                ui.small("Quality affects color and alpha. Even quality 100 is not guaranteed lossless; inspect the decoded preview.");
            }
            if dialog.options.format == files::RasterFormat::Heic {
                ui.small("HEIC uses the installed HEVC encoder. Quality affects compression; inspect the decoded preview for color and alpha changes.");
            }
            if dialog.options.format == files::RasterFormat::Heic {
                ui.small("Some Windows readers discard HEIC transparency or reject very small images. Use PNG for those workflows.");
            }
            if changed {
                dialog.preview.request(dialog.options);
                dialog.encoded=None;
                dialog.error=None;
            }
            if let Some(texture)=&dialog.texture {
                ui.add(egui::Image::new(texture).fit_to_original_size(1.0).max_width(ui.available_width()));
                ui.small("Decoded output preview (scaled to fit); checkerboard indicates transparency.");
            }
            if let Some(bytes)=&dialog.encoded {
                ui.label(format!("{} × {} px — {} bytes",dialog.dimensions.0,dialog.dimensions.1,bytes.len()));
            } else if let Some(error)=&dialog.error { ui.colored_label(ui.visuals().error_fg_color,error); }
            else { ui.label("Encoding preview…"); ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)); }
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(dialog.preview.ready && dialog.encoded.is_some() && (!dialog.save_document || dialog.flatten_confirmed),egui::Button::new(if dialog.save_document {"Save image"} else {"Save export"})).clicked() {
                    save=dialog.encoded.take(); keep=false;
                }
                if ui.button("Cancel").clicked() { keep=false; }
            });
        });
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            keep = false;
        }
        if let Some(bytes) = save {
            self.export_options = dialog.options;
            let path = dialog.path.clone();
            if dialog.save_document {
                self.save_raster_encoded(path, dialog.options.format, bytes);
            } else {
                self.start_cancellable_file_job("export image", move |cancel| {
                    files::save_encoded_raster(&bytes, &path, cancel)?;
                    Ok(Outcome::Exported(path))
                });
            }
        } else if keep && open {
            self.export_dialog = Some(dialog);
        } else if dialog.save_document {
            self.pending = self
                .after_save
                .take()
                .filter(|action| !matches!(action, Action::SaveAll));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raster_save_actions_remain_visible_below_scrolling_preview() {
        for size in [egui::vec2(800.0, 600.0), egui::vec2(640.0, 360.0)] {
            for format in [
                files::RasterFormat::Gif,
                files::RasterFormat::Jpeg,
                files::RasterFormat::JpegXl,
                files::RasterFormat::Avif,
                files::RasterFormat::Heic,
            ] {
                let ctx = egui::Context::default();
                let mut editor = Editor::blank(&ctx);
                editor.document = Document::new(512, 320, [40, 80, 120, 255]).unwrap();
                editor.begin_raster_save(
                    PathBuf::from("/tmp/example-output-image").with_extension(format.extension()),
                    format,
                );
                let input = || egui::RawInput {
                    screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size)),
                    ..Default::default()
                };
                let timeout = if format == files::RasterFormat::Avif {
                    30
                } else {
                    5
                };
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout);
                while !editor.export_dialog.as_ref().unwrap().preview.ready {
                    let _ = ctx.run(input(), |ctx| editor.show_export(ctx));
                    assert!(
                        std::time::Instant::now() < deadline,
                        "{format:?} export preview timed out"
                    );
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
                for _ in 0..2 {
                    let _ = ctx.run(input(), |ctx| editor.show_export(ctx));
                }
                let output = ctx.run(input(), |ctx| editor.show_export(ctx));
                let mut labels = Vec::new();
                for clipped in output.shapes {
                    if let egui::Shape::Text(text) = clipped.shape
                        && ["Save image", "Cancel"].contains(&text.galley.job.text.as_str())
                    {
                        let bounds = Rect::from_min_size(text.pos, text.galley.size());
                        assert!(
                            ctx.content_rect().contains_rect(bounds),
                            "{} outside window at {size:?}: {bounds:?}",
                            text.galley.job.text
                        );
                        assert!(
                            clipped.clip_rect.contains_rect(bounds),
                            "{} clipped at {size:?}",
                            text.galley.job.text
                        );
                        labels.push(text.galley.job.text.clone());
                    }
                }
                assert_eq!(labels, ["Save image", "Cancel"]);
                assert!(!editor.document.dirty());
            }
        }
    }
}
