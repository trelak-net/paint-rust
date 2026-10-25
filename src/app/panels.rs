use super::*;

#[derive(Clone, Copy)]
struct LayerDrag {
    document: u64,
    index: usize,
}

impl Editor {
    fn tools_panel(&mut self, ui: &mut egui::Ui) {
        if !self.panel_layout.floating {
            ui.heading("Tools");
        }
        ui.scope(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(4.0, 2.0);
            ui.spacing_mut().item_spacing.y = 3.0;
            let choices = [
                (Tool::Select, Icon::Select, "Select area", "S"),
                (Tool::Lasso, Icon::Lasso, "Lasso select", "Q"),
                (Tool::Polygon, Icon::Polygon, "Polygon select", "P"),
                (Tool::ColorSelect, Icon::ColorSelect, "Select color", "W"),
                (Tool::Move, Icon::Move, "Move", "M"),
                (Tool::Zoom, Icon::Zoom, "Zoom", "Z"),
                (Tool::Pan, Icon::Pan, "Pan", "H"),
                (Tool::Gradient, Icon::Gradient, "Gradient", "G"),
                (Tool::Text, Icon::Text, "Text", "T"),
                (Tool::Brush, Icon::Brush, "Brush", "B"),
                (Tool::Pencil, Icon::Pencil, "Pencil", "N"),
                (Tool::Eraser, Icon::Eraser, "Eraser", "E"),
                (Tool::Fill, Icon::Fill, "Flood fill", "F"),
                (Tool::Picker, Icon::Picker, "Color picker", "I"),
                (Tool::Line, Icon::Line, "Line", "L"),
                (Tool::Rectangle, Icon::Rectangle, "Rectangle", "R"),
                (Tool::Ellipse, Icon::Ellipse, "Ellipse", "O"),
            ];
            if self.panel_layout.floating {
                egui::Grid::new("compact-tools")
                    .num_columns(2)
                    .spacing(Vec2::splat(3.0))
                    .show(ui, |ui| {
                        for (index, (tool, icon, label, shortcut)) in choices.iter().enumerate() {
                            let response = ui.add_sized(
                                [32.0, 32.0],
                                icon.button("").selected(self.tool == *tool),
                            );
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(
                                    egui::WidgetType::Button,
                                    response.enabled(),
                                    self.tool == *tool,
                                    *label,
                                )
                            });
                            if response
                                .on_hover_text(format!("{label} ({shortcut})"))
                                .clicked()
                            {
                                self.tool = *tool;
                            }
                            if index % 2 == 1 {
                                ui.end_row();
                            }
                        }
                    });
                if let Some((_, _, label, shortcut)) =
                    choices.iter().find(|(tool, _, _, _)| *tool == self.tool)
                {
                    ui.label(format!("{label} ({shortcut})"));
                }
                return;
            }
            for (tool, icon, label, shortcut) in choices {
                let button = icon
                    .button(label)
                    .selected(self.tool == tool)
                    .shortcut_text(egui::RichText::new(shortcut).color(if self.tool == tool {
                        ui.visuals().selection.stroke.color
                    } else {
                        ui.visuals().weak_text_color()
                    }));
                if ui.add_sized([ui.available_width(), 26.0], button).clicked() {
                    self.tool = tool;
                }
            }
        });
    }
    fn colors_panel(&mut self, ui: &mut egui::Ui) {
        if !self.panel_layout.floating {
            ui.heading("Color");
        }
        ui.horizontal(|ui| {
            let label = ui.label("Primary");
            ui.color_edit_button_srgba(&mut self.color)
                .labelled_by(label.id);
        });
        ui.horizontal(|ui| {
            let label = ui.label("Secondary");
            ui.color_edit_button_srgba(&mut self.secondary_color)
                .labelled_by(label.id);
        });
        if ui.button("Swap colors    X").clicked() {
            std::mem::swap(&mut self.color, &mut self.secondary_color);
        }
        ui.horizontal_wrapped(|ui| {
            for color in self.color_panel.palette.clone() {
                let label = format!(
                    "Set primary color to #{:02X}{:02X}{:02X}",
                    color.r(),
                    color.g(),
                    color.b()
                );
                let response = ui.add(egui::Button::new("  ").fill(color));
                response.widget_info(|| {
                    egui::WidgetInfo::labeled(egui::WidgetType::Button, response.enabled(), &label)
                });
                let response = response.on_hover_text(format!(
                    "{label} (left click); set secondary with right click"
                ));
                if response.clicked() {
                    self.color = color;
                } else if response.clicked_by(egui::PointerButton::Secondary) {
                    self.secondary_color = color;
                }
            }
        });
        if ui.button("Reset colors").clicked() {
            self.color = Color32::BLACK;
            self.secondary_color = Color32::WHITE;
        }
        self.palette_actions(ui);
        self.color_channels(ui);
        ui.label(format!(
            "#{:02X}{:02X}{:02X}",
            self.color.r(),
            self.color.g(),
            self.color.b()
        ));
    }
    fn layers_panel(&mut self, ui: &mut egui::Ui) {
        if !self.panel_layout.floating {
            ui.heading("Layers");
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.document.can_add_layer(), egui::Button::new("Add"))
                .clicked()
            {
                self.document.add_layer(false);
                self.texture_dirty = true;
            }
            if ui
                .add_enabled(
                    self.document.can_add_layer(),
                    egui::Button::new("Duplicate"),
                )
                .clicked()
            {
                self.document.add_layer(true);
                self.texture_dirty = true;
            }
            if ui
                .add_enabled(
                    self.document.layers().len() > 1,
                    egui::Button::new("Delete"),
                )
                .clicked()
            {
                self.document.remove_layer();
                self.texture_dirty = true;
            }
        });
        ui.horizontal(|ui| {
            let active = self.document.active_layer();
            if ui
                .add_enabled(
                    active + 1 < self.document.layers().len(),
                    egui::Button::new("Move up"),
                )
                .clicked()
            {
                self.document.move_layer(true);
                self.texture_dirty = true;
            }
            if ui
                .add_enabled(active > 0, egui::Button::new("Move down"))
                .clicked()
            {
                self.document.move_layer(false);
                self.texture_dirty = true;
            }
        });
        if ui
            .add_enabled(
                self.document.can_add_layer(),
                egui::Button::new("Import image…"),
            )
            .on_hover_text("Add an image or a project's visible image as a new layer")
            .clicked()
        {
            self.import_layer();
        }
        if ui
            .add_enabled(
                self.document.can_merge_down(),
                egui::Button::new("Merge down"),
            )
            .on_hover_text("Merge into the layer below, retaining its properties (Ctrl+M)")
            .clicked()
        {
            self.start_document_edit(DocumentEdit::MergeDown);
        }
        if ui
            .add_enabled(
                self.document.can_flatten(),
                egui::Button::new("Flatten image"),
            )
            .on_hover_text("Replace all layers with the visible image. Undo restores the layers.")
            .clicked()
        {
            self.start_document_edit(DocumentEdit::Flatten);
        }
        ui.separator();
        self.layer_thumbnails.update(ui.ctx(), &self.document);
        let mut reorder = None;
        egui::ScrollArea::vertical()
            .id_salt("layer-list")
            .max_height(320.0)
            .show(ui, |ui| {
                for index in (0..self.document.layers().len()).rev() {
                    let layer = &self.document.layers()[index];
                    let mut visible = layer.visible;
                    let name = layer.name.clone();
                    ui.horizontal(|ui| {
                        let visibility = ui.checkbox(&mut visible, "");
                        let label = format!("Show layer {name}");
                        visibility.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::Checkbox,
                                visibility.enabled(),
                                visible,
                                &label,
                            )
                        });
                        if visibility.on_hover_text(label).changed() {
                            self.document.set_visibility(index, visible);
                            self.texture_dirty = true;
                        }
                        let response = ui
                            .push_id(index, |ui| {
                                ui.add(
                                    egui::Button::image_and_text(
                                        egui::Image::new((
                                            self.layer_thumbnails.texture(index),
                                            egui::vec2(40.0, 40.0),
                                        )),
                                        name,
                                    )
                                    .selected(index == self.document.active_layer())
                                    .sense(Sense::click_and_drag()),
                                )
                                .on_hover_text(
                                    "Drag onto another layer to change the stacking order",
                                )
                            })
                            .inner;
                        if response.clicked() {
                            self.document.select_layer(index);
                        }
                        response.dnd_set_drag_payload(LayerDrag {
                            document: self.active_document_id,
                            index,
                        });
                        if let Some(source) = response.dnd_release_payload::<LayerDrag>()
                            && source.document == self.active_document_id
                        {
                            reorder = Some((source.index, index));
                        }
                    });
                }
            });
        ui.separator();
        if let Some((source, target)) = reorder {
            self.texture_dirty |= self.document.reorder_layer(source, target);
        }
        let layer = &self.document.layers()[self.document.active_layer()];
        ui.label(format!(
            "Opacity: {:.0}%",
            f32::from(layer.opacity) * 100.0 / 255.0
        ));
        if ui.button("Layer properties…").clicked() {
            self.layer_properties = Some((layer.name.clone(), layer.opacity, layer.blend_mode));
        }
        if !layer.visible {
            ui.small("The active layer is hidden.");
        }
    }
    fn history_panel(&mut self, ui: &mut egui::Ui) {
        ui.small(format!(
            "RAM: {:.1} MiB; disk: {:.1} MiB",
            self.document.history_bytes() as f64 / (1024.0 * 1024.0),
            self.document.history_disk_bytes() as f64 / (1024.0 * 1024.0)
        ));
        if self.history_storage_paused {
            ui.small("History storage paused; undo remains in RAM.");
        }
        if (self.history_storage_paused || self.document.history_error().is_some())
            && ui.button("Resume history storage").clicked()
        {
            self.store_history_async();
        }
        let undo: Vec<String> = self.document.history().map(str::to_owned).collect();
        let redo: Vec<String> = self.document.redo_history().map(str::to_owned).collect();
        let mut chosen = None;
        egui::ScrollArea::vertical()
            .id_salt("history-list")
            .max_height(160.0)
            .show(ui, |ui| {
                if undo.is_empty() && redo.is_empty() {
                    ui.label("No edits yet");
                }
                for (index, label) in redo.iter().enumerate() {
                    if ui
                        .button(format!("Redo {label}"))
                        .on_hover_text(format!("Redo {} operation(s)", index + 1))
                        .clicked()
                    {
                        chosen = Some((index + 1, true));
                    }
                }
                if !undo.is_empty() && !redo.is_empty() {
                    ui.separator();
                }
                for (index, label) in undo.iter().enumerate() {
                    if ui
                        .button(format!("Undo {label}"))
                        .on_hover_text(format!("Undo {} operation(s)", index + 1))
                        .clicked()
                    {
                        chosen = Some((index + 1, false));
                    }
                }
            });
        if let Some((steps, redo)) = chosen {
            self.navigate_history(steps, redo);
        }
    }
}

pub(super) const NAMES: [&str; 4] = ["Tools", "History", "Layers", "Colors"];
pub(super) struct Layout {
    pub floating: bool,
    pub visible: [bool; 4],
    pub rects: [Option<Rect>; 4],
}
impl Default for Layout {
    fn default() -> Self {
        Self {
            floating: false,
            visible: [true; 4],
            rects: [None; 4],
        }
    }
}
impl Editor {
    fn panel_contents(&mut self, index: usize, ui: &mut egui::Ui, enabled: bool) {
        if !enabled || self.file_job.is_some() || self.document.editing() {
            ui.disable();
        }
        match index {
            0 => self.tools_panel(ui),
            1 => self.history_panel(ui),
            2 => self.layers_panel(ui),
            3 => self.colors_panel(ui),
            _ => unreachable!(),
        }
    }
    pub(super) fn workspace_panels(&mut self, ctx: &egui::Context, enabled: bool) {
        if self.panel_layout.floating {
            let area = ctx.available_rect();
            for (index, name) in NAMES.iter().enumerate() {
                let mut open = self.panel_layout.visible[index];
                let right = index == 1 || index == 2;
                let bottom = index == 2 || index == 3;
                let position = Pos2::new(
                    if right {
                        area.right() - 240.0
                    } else {
                        area.left() + 8.0
                    },
                    if bottom {
                        area.bottom() - 300.0
                    } else {
                        area.top() + 8.0
                    },
                );
                let rect = self.panel_layout.rects[index].unwrap_or(Rect::from_min_size(
                    position,
                    if index == 0 {
                        Vec2::new(100.0, 290.0)
                    } else {
                        Vec2::new(210.0, 260.0)
                    },
                ));
                let shown = egui::Window::new(*name)
                    .id(egui::Id::new(("workspace-panel", index, self.panel_epoch)))
                    .open(&mut open)
                    .default_pos(rect.min)
                    .default_size(rect.size())
                    .constrain_to(area)
                    .resizable(true)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt(("floating-content", index))
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                self.panel_contents(index, ui, enabled);
                            });
                        ui.min_size()
                    });
                self.panel_layout.visible[index] = open;
                if let Some(shown) = shown
                    && let Some(size) = shown.inner
                {
                    self.panel_layout.rects[index] = Some(Rect::from_min_size(
                        shown.response.rect.min,
                        size.clamp(Vec2::splat(40.0), Vec2::splat(4000.0)),
                    ));
                }
            }
        } else {
            if self.panel_layout.visible[0] || self.panel_layout.visible[3] {
                egui::SidePanel::left("tools")
                    .default_width(180.0)
                    .width_range(160.0..=240.0)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("tool-panel")
                            .show(ui, |ui| {
                                for index in [0, 3] {
                                    if self.panel_layout.visible[index] {
                                        self.panel_contents(index, ui, enabled);
                                        ui.separator();
                                    }
                                }
                            });
                    });
            }
            if self.panel_layout.visible[1] || self.panel_layout.visible[2] {
                egui::SidePanel::right("layers")
                    .default_width(220.0)
                    .width_range(180.0..=320.0)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("layers-panel")
                            .show(ui, |ui| {
                                if self.panel_layout.visible[2] {
                                    self.panel_contents(2, ui, enabled);
                                }
                                if self.panel_layout.visible[1] {
                                    ui.separator();
                                    egui::CollapsingHeader::new("History")
                                        .default_open(true)
                                        .show(ui, |ui| self.panel_contents(1, ui, enabled));
                                }
                            });
                    });
            }
        }
    }
}

impl Layout {
    pub(super) fn path() -> Option<PathBuf> {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|p| std::path::Path::new(p).is_absolute())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")))
            .map(|p| p.join("net.trelak.PaintRust/workspace.txt"))
    }
    fn encode(&self) -> String {
        let mut text = format!("workspace-v1\n{}\n", u8::from(self.floating));
        for index in 0..4 {
            text.push_str(&format!("{}", u8::from(self.visible[index])));
            if let Some(rect) = self.rects[index] {
                text.push_str(&format!(
                    " {} {} {} {}",
                    rect.min.x,
                    rect.min.y,
                    rect.width(),
                    rect.height()
                ));
            }
            text.push('\n');
        }
        text
    }
    fn decode(text: &str) -> Result<Self, String> {
        if text.len() > 4096 {
            return Err("Workspace settings exceed limit".into());
        }
        let mut lines = text.lines();
        if lines.next() != Some("workspace-v1") {
            return Err("Unsupported workspace settings".into());
        }
        let boolean = |s: &str| match s {
            "0" => Ok(false),
            "1" => Ok(true),
            _ => Err("Invalid panel flag".to_owned()),
        };
        let mut layout = Self {
            floating: boolean(lines.next().ok_or("Missing workspace mode")?)?,
            ..Default::default()
        };
        for index in 0..4 {
            let values: Vec<_> = lines
                .next()
                .ok_or("Missing panel settings")?
                .split_whitespace()
                .collect();
            if values.len() != 1 && values.len() != 5 {
                return Err("Invalid panel settings".into());
            }
            layout.visible[index] = boolean(values[0])?;
            if values.len() == 5 {
                let mut dimensions = [0.0f32; 4];
                for (target, value) in dimensions.iter_mut().zip(&values[1..]) {
                    *target = value.parse().map_err(|_| "Invalid panel coordinate")?;
                    if !target.is_finite() || target.abs() > 100000.0 {
                        return Err("Invalid panel coordinate".into());
                    }
                }
                if !(40.0..=4000.0).contains(&dimensions[2])
                    || !(40.0..=4000.0).contains(&dimensions[3])
                {
                    return Err("Invalid panel size".into());
                }
                layout.rects[index] = Some(Rect::from_min_size(
                    Pos2::new(dimensions[0], dimensions[1]),
                    Vec2::new(dimensions[2], dimensions[3]),
                ));
            }
        }
        if lines.next().is_some() {
            return Err("Unexpected workspace settings".into());
        }
        Ok(layout)
    }
    pub(super) fn load() -> Result<Self, String> {
        use std::io::Read;
        let Some(path) = Self::path() else {
            return Ok(Self::default());
        };
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e.to_string()),
        };
        let mut text = String::new();
        file.take(4097)
            .read_to_string(&mut text)
            .map_err(|e| e.to_string())?;
        Self::decode(&text)
    }
    pub(super) fn save(&self) -> Result<(), String> {
        use std::io::Write;
        let path = Self::path().ok_or("No configuration directory")?;
        let parent = path.parent().ok_or("Invalid configuration path")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let text = self.encode();
        Self::decode(&text)?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saved_layout_preserves_hidden_panels_and_positions() {
        let mut layout = Layout {
            floating: true,
            ..Default::default()
        };
        layout.visible[2] = false;
        layout.rects[0] = Some(Rect::from_min_size(
            Pos2::new(30.0, 50.0),
            Vec2::new(210.0, 320.0),
        ));
        let decoded = Layout::decode(&layout.encode()).unwrap();
        assert!(decoded.floating);
        assert_eq!(decoded.visible, layout.visible);
        assert_eq!(decoded.rects, layout.rects);
    }
    #[test]
    fn corrupt_layout_does_not_introduce_invisible_or_nonfinite_windows() {
        for value in ["NaN", "inf", "-1000000", "bad"] {
            let text = format!("workspace-v1\n1\n1 {value} 0 210 260\n1\n1\n1\n");
            assert!(Layout::decode(&text).is_err());
        }
        assert!(Layout::decode("workspace-v1\n1\n1 0 0 0 0\n1\n1\n1\n").is_err());
        assert!(Layout::decode("workspace-v1\n1\n1\n").is_err());
    }
}
