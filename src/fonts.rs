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
fn parse_catalog(text: &str) -> Result<Vec<InstalledFont>, String> {
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
        // Named variable-font instances need axis settings not supported by this renderer.
        if index > 65535 || parts[0].is_empty() || parts[0].len() > 512 || parts[1].len() > 512 {
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
    fonts.sort_by(|a, b| {
        (&a.family, &a.style, &a.path, a.index).cmp(&(&b.family, &b.style, &b.path, b.index))
    });
    fonts.dedup();
    Ok(fonts)
}

pub fn installed_cancellable(cancel: &AtomicBool) -> Result<Vec<InstalledFont>, String> {
    use std::{
        io::Read,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    if cancel.load(Ordering::Relaxed) {
        return Err(crate::files::CANCELLED.into());
    }
    let mut child = Command::new("fc-list")
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
    parse_catalog(std::str::from_utf8(&bytes).map_err(|_| "Font catalog is not valid UTF-8")?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parser_keeps_face_identity_styles_and_paths_with_spaces() {
        let text = "Example\tBold\t/usr/share/fonts/a b.ttc\t2\nExample\tRegular\t/usr/share/fonts/a b.ttc\t0\nExample\tBold\t/usr/share/fonts/a b.ttc\t2\nVariable\tLight\t/usr/share/fonts/v.ttf\t65536\nBitmap\tNormal\t/usr/share/fonts/b.pcf\t0\nRelative\tNormal\tb.ttf\t0\nbroken";
        let fonts = parse_catalog(text).unwrap();
        assert_eq!(fonts.len(), 2);
        assert_eq!(fonts[0].style, "Bold");
        assert_eq!(fonts[0].index, 2);
        assert_eq!(fonts[0].path, PathBuf::from("/usr/share/fonts/a b.ttc"));
        assert!(parse_catalog(&"x".repeat(4 * 1024 * 1024 + 1)).is_err());
        assert!(installed_cancellable(&AtomicBool::new(true)).is_err());
    }
}
