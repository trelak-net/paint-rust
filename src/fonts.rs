//! Bounded discovery of installed outline fonts through the system catalog.
use std::{
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledFont {
    pub family: String,
    pub style: String,
    pub path: PathBuf,
    pub index: u32,
}
impl InstalledFont {
    pub fn label(&self) -> String {
        format!("{} — {}", self.family, self.style)
    }
}
fn parse_catalog_ordered(text: &str) -> Result<Vec<InstalledFont>, String> {
    parse_catalog_instances(text, false)
}
fn parse_catalog_instances(text: &str, instances: bool) -> Result<Vec<InstalledFont>, String> {
    if text.len() > 4 * 1024 * 1024 {
        return Err("System font catalog exceeds 4 MiB.".into());
    }
    let mut fonts = Vec::new();
    for line in text.lines() {
        let parts: Vec<_> = line.split('\t').collect();
        if parts.len() != 4 {
            continue;
        }
        let Ok(index) = parts[3].parse::<u32>() else {
            continue;
        };
        // The font browser exposes base faces; fallback resolves named-instance axes.
        if index > 0x7fff_ffff
            || (!instances && index > 65535)
            || parts[0].is_empty()
            || parts[0].len() > 512
            || parts[1].len() > 512
        {
            continue;
        }
        let path = PathBuf::from(parts[2]);
        if !path.is_absolute()
            || !path.extension().is_some_and(|e| {
                ["ttf", "otf", "ttc", "otc"]
                    .iter()
                    .any(|ext| e.eq_ignore_ascii_case(ext))
            })
        {
            continue;
        }
        fonts.push(InstalledFont {
            family: parts[0].into(),
            style: parts[1].into(),
            path,
            index,
        });
        if fonts.len() > 16384 {
            return Err("System font catalog exceeds 16384 faces.".into());
        }
    }
    let mut seen = std::collections::HashSet::new();
    fonts.retain(|font| seen.insert((font.path.clone(), font.index)));
    Ok(fonts)
}

fn parse_catalog(text: &str) -> Result<Vec<InstalledFont>, String> {
    let mut fonts = parse_catalog_ordered(text)?;
    fonts.sort_by(|a, b| {
        (&a.family, &a.style, &a.path, a.index).cmp(&(&b.family, &b.style, &b.path, b.index))
    });
    Ok(fonts)
}

pub fn installed_cancellable(cancel: &AtomicBool) -> Result<Vec<InstalledFont>, String> {
    parse_catalog(&query("fc-list", &[], cancel)?)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FallbackStyle {
    pub weight: u16,
    pub slant: u16,
}
impl FallbackStyle {
    pub fn from_face(face: &rustybuzz::Face<'_>, axes: crate::text::variations::Settings) -> Self {
        // Fontconfig weight classes differ from OpenType's 100..900 scale.
        let weight = match axes
            .get(*b"wght")
            .unwrap_or(f32::from(face.weight().to_number()))
            .clamp(1.0, 1000.0) as u16
        {
            0..=150 => 0,
            151..=250 => 40,
            251..=325 => 50,
            326..=365 => 55,
            366..=390 => 75,
            391..=450 => 80,
            451..=550 => 100,
            551..=650 => 180,
            651..=750 => 200,
            751..=850 => 205,
            _ => 210,
        };
        let slant = if let Some(italic) = axes.get(*b"ital") {
            if italic >= 0.5 { 100 } else { 0 }
        } else if let Some(slant) = axes.get(*b"slnt") {
            if slant != 0.0 { 110 } else { 0 }
        } else if face.is_oblique() {
            110
        } else if face.is_italic() {
            100
        } else {
            0
        };
        Self { weight, slant }
    }
}
fn fallback_pattern(family: &str, style: FallbackStyle) -> String {
    let mut escaped = String::new();
    for ch in family.chars().filter(|ch| !ch.is_control()).take(512) {
        if matches!(ch, '\\' | ':' | ',' | '=') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    if escaped.is_empty() {
        escaped.push_str("sans-serif");
    }
    format!(
        ":family={escaped}:weight={}:slant={}",
        style.weight, style.slant
    )
}
pub(crate) fn fallback_candidates(
    family: &str,
    style: FallbackStyle,
    cancel: &AtomicBool,
) -> Result<Vec<InstalledFont>, String> {
    let pattern = fallback_pattern(family, style);
    parse_catalog_instances(&query("fc-match", &["-s", &pattern], cancel)?, true)
}

fn query(program: &str, arguments: &[&str], cancel: &AtomicBool) -> Result<String, String> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if cancel.load(Ordering::Relaxed) {
        return Err(crate::files::CANCELLED.into());
    }
    let mut child = Command::new(program)
        .args(arguments)
        .args(["--format", "%{family[0]}\t%{style[0]}\t%{file}\t%{index}\n"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not query installed fonts ({e}). Load a font file instead."))?;
    let stdout = child.stdout.take().ok_or("No font catalog output")?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let deadline = Instant::now() + Duration::from_secs(15);
    let result = loop {
        if cancel.load(Ordering::Relaxed) {
            break Err(crate::files::CANCELLED.to_string());
        }
        if Instant::now() > deadline {
            break Err("System font discovery timed out.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                break if status.success() {
                    Ok(())
                } else {
                    Err("System font query failed.".into())
                };
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(10)),
            Err(error) => break Err(error.to_string()),
        }
    };
    if result.is_err() {
        let _ = child.kill();
    }
    let _ = child.wait();
    let bytes = reader
        .join()
        .map_err(|_| "Font catalog reader failed")?
        .map_err(|e| e.to_string())?;
    result?;
    if cancel.load(Ordering::Relaxed) {
        return Err(crate::files::CANCELLED.into());
    }
    String::from_utf8(bytes).map_err(|_| "Font catalog is not valid UTF-8".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallback_pattern_keeps_family_data_separate_from_style_properties() {
        let style = FallbackStyle {
            weight: 200,
            slant: 100,
        };
        assert_eq!(
            fallback_pattern("Example", style),
            ":family=Example:weight=200:slant=100"
        );
        assert_eq!(
            fallback_pattern("", style),
            ":family=sans-serif:weight=200:slant=100"
        );
        assert_eq!(
            fallback_pattern("-Demo:weight=0,Other\n", style),
            r":family=-Demo\:weight\=0\,Other:weight=200:slant=100"
        );
        assert!(fallback_pattern(&"x".repeat(10000), style).len() < 600);
    }

    #[test]
    #[ignore = "requires a host Fontconfig installation with static Open Sans styles"]
    fn native_fontconfig_honors_regular_and_bold_italic_preferences() {
        let cancel = AtomicBool::new(false);
        let regular = fallback_candidates(
            "Open Sans",
            FallbackStyle {
                weight: 80,
                slant: 0,
            },
            &cancel,
        )
        .unwrap();
        let bold = fallback_candidates(
            "Open Sans",
            FallbackStyle {
                weight: 200,
                slant: 100,
            },
            &cancel,
        )
        .unwrap();
        let read = |candidate: &InstalledFont| {
            let bytes = std::fs::read(&candidate.path).unwrap();
            let face = rustybuzz::Face::from_slice(&bytes, candidate.index).unwrap();
            (
                face.weight().to_number(),
                face.is_italic() || face.is_oblique(),
            )
        };
        let normal = read(&regular[0]);
        let heavy = read(&bold[0]);
        assert!(heavy.0 > normal.0);
        assert!(!normal.1);
        assert!(heavy.1);
    }

    #[test]
    fn parser_keeps_face_identity_styles_and_paths_with_spaces() {
        let text = "Example\tBold\t/usr/share/fonts/a b.ttc\t2\nExample\tRegular\t/usr/share/fonts/a b.ttc\t0\nExample\tBold\t/usr/share/fonts/a b.ttc\t2\nVariable\tLight\t/usr/share/fonts/v.ttf\t65536\nBitmap\tNormal\t/usr/share/fonts/b.pcf\t0\nRelative\tNormal\tb.ttf\t0\nbroken";
        let instances = parse_catalog_instances(text, true).unwrap();
        assert!(instances.iter().any(|font| font.index == 65536));
        let fonts = parse_catalog(text).unwrap();
        assert_eq!(fonts.len(), 2);
        assert_eq!(fonts[0].style, "Bold");
        assert_eq!(fonts[0].index, 2);
        assert_eq!(fonts[0].path, PathBuf::from("/usr/share/fonts/a b.ttc"));
        assert!(parse_catalog(&"x".repeat(4 * 1024 * 1024 + 1)).is_err());
        assert!(installed_cancellable(&AtomicBool::new(true)).is_err());
    }
}
