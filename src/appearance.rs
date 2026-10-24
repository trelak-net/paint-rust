use eframe::egui::{self, Color32, Theme};

pub fn configure(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::System);
    ctx.options_mut(|options| options.fallback_theme = Theme::Light);
    for theme in [Theme::Light, Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(12.0, 6.0);
            style.spacing.interact_size.y = 30.0;
            style.visuals.widgets.inactive.corner_radius = 6.into();
            style.visuals.widgets.hovered.corner_radius = 6.into();
            style.visuals.widgets.active.corner_radius = 6.into();
            style.visuals.selection.bg_fill = Color32::from_rgb(53, 132, 228);
            style.visuals.selection.stroke.color = Color32::WHITE;
            style.visuals.panel_fill = match theme {
                Theme::Light => Color32::from_rgb(246, 245, 244),
                Theme::Dark => Color32::from_rgb(36, 36, 36),
            };
        });
    }
}

pub fn workspace_color(theme: Theme) -> Color32 {
    match theme {
        Theme::Light => Color32::from_rgb(222, 221, 218),
        Theme::Dark => Color32::from_rgb(30, 30, 30),
    }
}

pub fn connect(ctx: &egui::Context) -> std::sync::mpsc::Receiver<Theme> {
    let (sender, receiver) = std::sync::mpsc::channel();
    #[cfg(target_os = "linux")]
    {
        load_desktop_font(ctx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            use ashpd::desktop::settings::{ColorScheme, Settings};
            use futures_lite::StreamExt;
            let publish = |scheme| {
                let theme = match scheme {
                    ColorScheme::PreferDark => Theme::Dark,
                    _ => Theme::Light,
                };
                let alive = sender.send(theme).is_ok();
                ctx.request_repaint();
                alive
            };
            futures_lite::future::block_on(async {
                let Ok(settings) = Settings::new().await else {
                    return;
                };
                let Ok(mut changes) = settings.receive_color_scheme_changed().await else {
                    return;
                };
                if let Ok(scheme) = settings.color_scheme().await {
                    publish(scheme);
                }
                while let Some(scheme) = changes.next().await {
                    if !publish(scheme) {
                        break;
                    }
                }
            });
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (sender, ctx);
    receiver
}

#[cfg(target_os = "linux")]
fn load_desktop_font(ctx: &egui::Context) {
    use std::process::Command;
    let description = Command::new("gsettings")
        .args(["get", "org.gnome.desktop.interface", "font-name"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok());
    let (family, size) = parse_font(description.as_deref().unwrap_or("sans-serif 11"));
    let Some(path) = Command::new("fc-match")
        .args(["-f", "%{file}", &family])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
    else {
        return;
    };
    let Ok(bytes) = std::fs::read(path.trim()) else {
        return;
    };
    let mut fonts = egui::FontDefinitions::default();
    fonts
        .font_data
        .insert("desktop".into(), egui::FontData::from_owned(bytes).into());
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "desktop".into());
    ctx.set_fonts(fonts);
    for theme in [Theme::Light, Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            for text in [egui::TextStyle::Body, egui::TextStyle::Button] {
                style
                    .text_styles
                    .insert(text, egui::FontId::proportional(size));
            }
            style.text_styles.insert(
                egui::TextStyle::Small,
                egui::FontId::proportional((size - 2.0).max(10.0)),
            );
            style.text_styles.insert(
                egui::TextStyle::Heading,
                egui::FontId::proportional(size + 3.0),
            );
        });
    }
}

#[cfg(any(target_os = "linux", test))]
fn parse_font(description: &str) -> (String, f32) {
    let value = description.trim().trim_matches('\'');
    if let Some((family, size)) = value.rsplit_once(' ')
        && let Ok(points) = size.parse::<f32>()
        && points.is_finite()
        && points > 0.0
    {
        return (family.to_owned(), (points * 96.0 / 72.0).clamp(10.0, 32.0));
    }
    ("sans-serif".into(), 14.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn system_theme_changes_panels_and_workspace_together() {
        let ctx = egui::Context::default();
        configure(&ctx);
        ctx.set_theme(Theme::Light);
        assert!(!ctx.style().visuals.dark_mode);
        let light_panel = ctx.style().visuals.panel_fill;
        ctx.set_theme(Theme::Dark);
        assert!(ctx.style().visuals.dark_mode);
        assert_ne!(light_panel, ctx.style().visuals.panel_fill);
        assert_ne!(workspace_color(Theme::Light), workspace_color(Theme::Dark));
    }
    #[test]
    fn desktop_font_description_uses_points_without_double_scaling() {
        assert_eq!(parse_font("'Adwaita Sans 11'\n").0, "Adwaita Sans");
        assert!((parse_font("'Adwaita Sans 11'").1 - 14.666667).abs() < 0.001);
        assert_eq!(parse_font("bad input"), ("sans-serif".into(), 14.0));
        assert_eq!(parse_font("Sans NaN"), ("sans-serif".into(), 14.0));
    }
}
