//! Шрифты интерфейса: встроенный Noto Sans и пользовательские TTF/OTF из
//! папки `fonts` рядом с exe.

use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;

pub struct FontFace {
    pub name: String,
    pub bytes: Vec<u8>,
}

fn fonts_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("fonts")))
        .unwrap_or_else(|| PathBuf::from("fonts"))
}

pub fn prepare() {
    let _ = std::fs::create_dir_all(fonts_dir());
}

pub fn load() -> Vec<FontFace> {
    prepare();
    // Noto Sans — единственный встроенный шрифт: высокий x-height и полное
    // покрытие латиницы (включая турецкие буквы) и кириллицы, поэтому мелкие
    // подписи читаются хорошо. Остальные шрифты — из папки `fonts`.
    let mut faces = vec![FontFace {
        name: "Noto Sans".to_string(),
        bytes: include_bytes!("../fonts/noto_sans.ttf").to_vec(),
    }];
    let mut custom = Vec::new();
    if let Ok(entries) = std::fs::read_dir(fonts_dir()) {
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ext != "ttf" && ext != "otf" {
                continue;
            }
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("font")
                .to_string();
            // Встроенные шрифты не дублируем, даже если файл лежит в папке.
            if stem.eq_ignore_ascii_case("noto_sans") {
                continue;
            }
            if let Ok(bytes) = std::fs::read(&path) {
                if bytes.len() > 12 {
                    custom.push(FontFace {
                        name: stem,
                        bytes,
                    });
                }
            }
        }
    }
    custom.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    faces.extend(custom);
    faces
}

pub fn apply(ctx: &egui::Context, faces: &[FontFace], selected: usize) {
    let Some(selected_face) = faces.get(selected).or_else(|| faces.first()) else {
        return;
    };
    let mut defs = egui::FontDefinitions::default();
    for face in faces {
        defs.font_data.insert(
            face.name.clone(),
            Arc::new(egui::FontData::from_owned(face.bytes.clone())),
        );
    }
    for family in [
        egui::FontFamily::Proportional,
        egui::FontFamily::Monospace,
    ] {
        let list = defs.families.entry(family).or_default();
        list.insert(0, selected_face.name.clone());
    }
    ctx.set_fonts(defs);
}
