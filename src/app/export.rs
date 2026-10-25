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
        egui::Window::new(format!("{} {}",if dialog.save_document {"Save"} else {"Export"},dialog.options.format.label())).collapsible(false).resizable(true).vscroll(true).max_height((ctx.content_rect().height()-80.0).max(180.0)).default_width(540.0).open(&mut open).show(ctx,|ui| {
            ui.add(egui::Label::new(dialog.path.display().to_string()).truncate()).on_hover_text(dialog.path.display().to_string());
            if dialog.save_document {
                ui.label("Saving as a raster replaces the current document with the flattened decoded image. You can undo this after saving.");
                ui.small("Layers and metadata are not stored in this file. Use PRUST or PDN to keep an editable project. JPEG also loses transparency and image detail.");
                ui.checkbox(&mut dialog.flatten_confirmed,"Save a flattened raster document");
            }
            let mut changed=false;
            if dialog.options.format==files::RasterFormat::Jpeg {
                changed |= ui.add(egui::Slider::new(&mut dialog.options.quality,1..=100).text("Quality")).changed();
            }
            if matches!(dialog.options.format,files::RasterFormat::Jpeg | files::RasterFormat::Bmp) {
                ui.horizontal(|ui| { ui.label("Transparency background"); changed |= ui.color_edit_button_srgb(&mut dialog.options.matte).changed(); });
                ui.small("Transparency is composited onto this color before encoding.");
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
            self.pending = self.after_save.take();
        }
    }
}
