//! Локализация интерфейса.
//!
//! Переводы лежат в папке `lang` рядом с игрой:
//! `lang/ru.po`, `lang/en.po`, `lang/es.po`, `lang/de.po`, `lang/fr.po`,
//! `lang/tr.po` (или скомпилированные `.mo`). Формат — обычный gettext:
//! `msgid` — русская строка, `msgstr` — перевод. Русский каталог можно не
//! заполнять: он показывает исходные строки.
//!
//! Выбранный язык запоминается в `lang/current.txt`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Ru,
    En,
    Es,
    De,
    Fr,
    Tr,
}

impl Lang {
    pub const ALL: [Lang; 6] = [
        Lang::Ru,
        Lang::En,
        Lang::Es,
        Lang::De,
        Lang::Fr,
        Lang::Tr,
    ];

    pub fn code(self) -> &'static str {
        match self {
            Lang::Ru => "ru",
            Lang::En => "en",
            Lang::Es => "es",
            Lang::De => "de",
            Lang::Fr => "fr",
            Lang::Tr => "tr",
        }
    }

    /// Короткая подпись для кнопки в настройках.
    pub fn label(self) -> &'static str {
        match self {
            Lang::Ru => "RU",
            Lang::En => "EN",
            Lang::Es => "ES",
            Lang::De => "DE",
            Lang::Fr => "FR",
            Lang::Tr => "TR",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        match code.trim().to_ascii_lowercase().as_str() {
            "ru" => Some(Lang::Ru),
            "en" => Some(Lang::En),
            "es" => Some(Lang::Es),
            "de" => Some(Lang::De),
            "fr" => Some(Lang::Fr),
            "tr" => Some(Lang::Tr),
            _ => None,
        }
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);
static CATALOG: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
static EN_CATALOG: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
static CACHE: OnceLock<Mutex<HashMap<(u8, &'static str), &'static str>>> = OnceLock::new();

fn catalog() -> &'static Mutex<HashMap<String, String>> {
    CATALOG.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Английский каталог: журнал партии всегда пишется по-английски,
/// кроме русской локализации, где он остаётся русским.
fn en_catalog() -> &'static Mutex<HashMap<String, String>> {
    EN_CATALOG.get_or_init(|| Mutex::new(load_catalog(Lang::En)))
}

fn cache() -> &'static Mutex<HashMap<(u8, &'static str), &'static str>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Папка с переводами: рядом с исполняемым файлом, иначе текущая.
pub fn lang_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("lang");
            if p.is_dir() {
                return p;
            }
        }
    }
    PathBuf::from("lang")
}

/// Текущий язык.
pub fn current() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        1 => Lang::En,
        2 => Lang::Es,
        3 => Lang::De,
        4 => Lang::Fr,
        5 => Lang::Tr,
        _ => Lang::Ru,
    }
}

/// Загружает язык из `lang/current.txt` (по умолчанию русский).
pub fn init() {
    let lang = std::fs::read_to_string(lang_dir().join("current.txt"))
        .ok()
        .and_then(|s| Lang::from_code(&s))
        .unwrap_or(Lang::Ru);
    set_lang(lang);
}

/// Переключает язык и перезагружает каталог.
pub fn set_lang(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
    let map = load_catalog(lang);
    *catalog().lock().unwrap() = map;
    let _ = std::fs::create_dir_all(lang_dir());
    let _ = std::fs::write(lang_dir().join("current.txt"), lang.code());
}

/// Перевод строки-идентификатора. Для русского возвращает исходную строку.
pub fn tr(msgid: &'static str) -> &'static str {
    let lang = current();
    if lang == Lang::Ru {
        return msgid;
    }
    let key = (lang as u8, msgid);
    if let Some(v) = cache().lock().unwrap().get(&key).copied() {
        return v;
    }
    let translated = catalog().lock().unwrap().get(msgid).cloned();
    match translated {
        Some(t) => {
            let leaked: &'static str = Box::leak(t.into_boxed_str());
            cache().lock().unwrap().insert(key, leaked);
            leaked
        }
        None => msgid,
    }
}

/// Перевод уже собранной строки (для текста, нарисованного на канвасе).
pub fn tr_owned(s: String) -> String {
    if current() == Lang::Ru {
        return s;
    }
    if let Some(t) = catalog().lock().unwrap().get(&s) {
        return t.clone();
    }
    s
}

/// Перевод уже собранной строки для журнала: ru для русской локализации,
/// en для остальных.
pub fn log_owned(s: String) -> String {
    if log_lang() == Lang::Ru {
        return s;
    }
    if let Some(t) = en_catalog().lock().unwrap().get(&s) {
        return t.clone();
    }
    s
}

/// Перевод с подстановкой именованных частей: `{name}`.
pub fn trf(msgid: &'static str, args: &[(&str, &str)]) -> String {
    let mut s = tr(msgid).to_string();
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

/// Язык журнала партии: русский для русской локализации, английский для всех
/// остальных. Так логи остаются читаемыми для разработчика и не смешиваются
/// с языком интерфейса.
pub fn log_lang() -> Lang {
    if current() == Lang::Ru {
        Lang::Ru
    } else {
        Lang::En
    }
}

/// Перевод строки журнала по правилу: ru для русской локализации, en для всех
/// остальных. Для русского возвращает исходную строку.
pub fn log(msgid: &'static str) -> &'static str {
    if log_lang() == Lang::Ru {
        return msgid;
    }
    let key = (Lang::En as u8, msgid);
    if let Some(v) = cache().lock().unwrap().get(&key).copied() {
        return v;
    }
    let translated = en_catalog().lock().unwrap().get(msgid).cloned();
    match translated {
        Some(t) => {
            let leaked: &'static str = Box::leak(t.into_boxed_str());
            cache().lock().unwrap().insert(key, leaked);
            leaked
        }
        None => msgid,
    }
}

/// Перевод строки журнала с подстановкой именованных частей: `{name}`.
pub fn logf(msgid: &'static str, args: &[(&str, &str)]) -> String {
    let mut s = log(msgid).to_string();
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), v);
    }
    s
}

// ---------------------------------------------------------------------------
// Каталоги
// ---------------------------------------------------------------------------

fn load_catalog(lang: Lang) -> HashMap<String, String> {
    if lang == Lang::Ru {
        return HashMap::new();
    }
    let dir = lang_dir();
    let po = dir.join(format!("{}.po", lang.code()));
    let mo = dir.join(format!("{}.mo", lang.code()));
    if po.is_file() {
        if let Ok(text) = std::fs::read_to_string(&po) {
            return parse_po(&text);
        }
    }
    if mo.is_file() {
        if let Ok(bytes) = std::fs::read(&mo) {
            return parse_mo(&bytes);
        }
    }
    HashMap::new()
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Разбирает .po-файл gettext.
pub fn parse_po(text: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut msgid: Option<String> = None;
    let mut msgstr: Option<String> = None;
    let mut last: Option<bool> = None; // true — msgid, false — msgstr
    let mut skip = false;
    // Флаг fuzzy относится к следующей записи, а не к текущей.
    let mut pending_skip = false;

    fn flush(
        map: &mut HashMap<String, String>,
        msgid: &mut Option<String>,
        msgstr: &mut Option<String>,
        skip: &mut bool,
    ) {
        if let (Some(id), Some(st)) = (msgid.take(), msgstr.take()) {
            if !*skip && !id.is_empty() && !st.is_empty() {
                map.insert(id, st);
            }
        }
        *msgid = None;
        *msgstr = None;
        *skip = false;
    }

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            flush(&mut map, &mut msgid, &mut msgstr, &mut skip);
            last = None;
            continue;
        }
        if line.starts_with('#') {
            // Флаг fuzzy означает, что перевод ещё не готов.
            if line.contains("fuzzy") {
                pending_skip = true;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("msgid ") {
            flush(&mut map, &mut msgid, &mut msgstr, &mut skip);
            skip = pending_skip;
            pending_skip = false;
            msgid = Some(unescape(rest.trim_matches('"')));
            last = Some(true);
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            msgstr = Some(unescape(rest.trim_matches('"')));
            last = Some(false);
        } else if line.starts_with('"') {
            let part = unescape(line.trim_matches('"'));
            match last {
                Some(true) => {
                    if let Some(v) = msgid.as_mut() {
                        v.push_str(&part);
                    }
                }
                Some(false) => {
                    if let Some(v) = msgstr.as_mut() {
                        v.push_str(&part);
                    }
                }
                None => {}
            }
        }
    }
    flush(&mut map, &mut msgid, &mut msgstr, &mut skip);
    map
}

/// Разбирает скомпилированный .mo-файл gettext.
pub fn parse_mo(bytes: &[u8]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    if bytes.len() < 28 {
        return map;
    }
    let u32_at = |off: usize| -> Option<u32> {
        let b = bytes.get(off..off + 4)?;
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let magic = u32_at(0).unwrap_or(0);
    if magic != 0x9504_12de {
        return map;
    }
    let count = u32_at(8).unwrap_or(0) as usize;
    let orig_off = u32_at(12).unwrap_or(0) as usize;
    let trans_off = u32_at(16).unwrap_or(0) as usize;
    let read_str = |table: usize, i: usize| -> Option<String> {
        let len = u32_at(table + i * 8)? as usize;
        let off = u32_at(table + i * 8 + 4)? as usize;
        let slice = bytes.get(off..off + len)?;
        Some(String::from_utf8_lossy(slice).into_owned())
    };
    for i in 0..count {
        if let (Some(id), Some(st)) = (read_str(orig_off, i), read_str(trans_off, i)) {
            if !id.is_empty() && !st.is_empty() {
                map.insert(id, st);
            }
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn po_parses_simple_entries() {
        let src = r#"
msgid ""
msgstr ""
"Content-Type: text/plain; charset=UTF-8\n"

msgid "Настройки"
msgstr "Settings"

msgid "Ход {n}"
msgstr "Move {n}"
"#;
        let map = parse_po(src);
        assert_eq!(map.get("Настройки").map(String::as_str), Some("Settings"));
        assert_eq!(map.get("Ход {n}").map(String::as_str), Some("Move {n}"));
    }

    #[test]
    fn po_skips_fuzzy() {
        let src = "#, fuzzy\nmsgid \"Правила\"\nmsgstr \"Rules\"\n";
        let map = parse_po(src);
        assert!(map.is_empty());
    }

    #[test]
    fn lang_codes_round_trip() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_code(lang.code()), Some(lang));
        }
    }

    #[test]
    fn bundled_po_covers_all_msgids() {
        let Ok(msgids) = std::fs::read_to_string("lang/msgids.txt") else {
            return; // в собранной игре каталога исходников может не быть
        };
        let expected: Vec<String> = msgids.lines().map(unescape).collect();
        for lang in [Lang::En, Lang::Es, Lang::De, Lang::Fr, Lang::Tr] {
            let path = format!("lang/{}.po", lang.code());
            let Ok(text) = std::fs::read_to_string(&path) else {
                panic!("нет файла {path}");
            };
            let map = parse_po(&text);
            let missing: Vec<&str> = expected
                .iter()
                .filter(|id| !map.contains_key(*id))
                .map(String::as_str)
                .collect();
            assert!(
                missing.is_empty(),
                "{}: нет переводов для {:?}",
                lang.code(),
                missing
            );
        }
    }
}
