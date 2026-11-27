//! Проверка обновлений: страница последнего релиза на GitHub.
//!
//! Запрос идёт через системный WinHTTP — никаких новых крейтов и внешних
//! процессов: для антивируса это обычный HTTPS-запрос стандартной библиотеки
//! Windows. Игра ничего не скачивает и не заменяет: по клику открывается
//! страница релиза в браузере, установку делает сам пользователь.

/// Версия этой сборки («0.18.0»).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const RELEASE_URL: &str = "https://github.com/aleandr1tester/shatmat/releases/latest";

/// Результат фоновой проверки: версия последнего релиза или причина отказа.
pub type CheckResult = Result<String, String>;

/// Состояние кнопки обновлений в настройках.
#[derive(Clone, PartialEq, Debug)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate,
    Available(String),
    Failed,
}

/// true, если remote новее local («0.18.1» против «0.18.0»).
pub fn is_newer(remote: &str, local: &str) -> bool {
    match (parse_ver(remote), parse_ver(local)) {
        (Some(r), Some(l)) => r > l,
        _ => false,
    }
}

fn parse_ver(s: &str) -> Option<Vec<u32>> {
    let s = s.trim().trim_start_matches('v').trim();
    if s.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for part in s.split('.') {
        out.push(part.trim().parse::<u32>().ok()?);
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}

/// Запросить версию последнего релиза. Блокирующий вызов — держать в потоке.
///
/// WinINet берёт прокси из системных настроек так же, как браузер и сама
/// Windows: работает и на обычных машинах, и там, где настроен локальный
/// прокси. Редиректы WinINet проходит сам, а итоговый адрес отдаёт через
/// INTERNET_OPTION_URL — так версия узнаётся без разбора JSON:
/// «…/releases/latest» → «…/releases/tag/v0.18.1».
///
/// Импорт объявлен через `raw-dylib`: линковать сторонние библиотеки не
/// нужно, зависимость — только системная wininet.dll из состава Windows.
#[cfg(windows)]
pub fn check_latest() -> CheckResult {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "wininet", kind = "raw-dylib")]
    extern "system" {
        fn InternetOpenW(
            agent: *const u16,
            access: u32,
            proxy: *const u16,
            bypass: *const u16,
            flags: u32,
        ) -> *mut core::ffi::c_void;
        fn InternetOpenUrlW(
            h: *mut core::ffi::c_void,
            url: *const u16,
            headers: *const u16,
            headers_len: u32,
            flags: u32,
            context: usize,
        ) -> *mut core::ffi::c_void;
        fn InternetQueryOptionW(
            h: *mut core::ffi::c_void,
            option: u32,
            buffer: *mut core::ffi::c_void,
            len: *mut u32,
        ) -> i32;
        fn InternetSetOptionW(
            h: *mut core::ffi::c_void,
            option: u32,
            buffer: *const core::ffi::c_void,
            len: u32,
        ) -> i32;
        fn InternetCloseHandle(h: *mut core::ffi::c_void) -> i32;
    }

    const INTERNET_OPEN_TYPE_PRECONFIG: u32 = 0;
    const INTERNET_OPTION_URL: u32 = 34;
    const INTERNET_OPTION_CONNECT_TIMEOUT: u32 = 2;
    const INTERNET_OPTION_RECEIVE_TIMEOUT: u32 = 6;
    const INTERNET_FLAG_RELOAD: u32 = 0x8000_0000;
    const INTERNET_FLAG_NO_CACHE_WRITE: u32 = 0x0400_0000;
    const INTERNET_FLAG_SECURE: u32 = 0x0080_0000;

    let wide = |s: &str| -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };

    unsafe {
        let agent = wide("ShatMat");
        let session = InternetOpenW(
            agent.as_ptr(),
            INTERNET_OPEN_TYPE_PRECONFIG,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if session.is_null() {
            return Err(win_err("InternetOpen"));
        }
        let timeout: u32 = 6000;
        InternetSetOptionW(
            session,
            INTERNET_OPTION_CONNECT_TIMEOUT,
            &timeout as *const u32 as *const _,
            4,
        );
        InternetSetOptionW(
            session,
            INTERNET_OPTION_RECEIVE_TIMEOUT,
            &timeout as *const u32 as *const _,
            4,
        );
        let url = wide("https://github.com/aleandr1tester/shatmat/releases/latest");
        let req = InternetOpenUrlW(
            session,
            url.as_ptr(),
            std::ptr::null(),
            0,
            INTERNET_FLAG_RELOAD | INTERNET_FLAG_NO_CACHE_WRITE | INTERNET_FLAG_SECURE,
            0,
        );
        if req.is_null() {
            let e = win_err("InternetOpenUrl");
            InternetCloseHandle(session);
            return Err(e);
        }
        let mut len: u32 = 0;
        InternetQueryOptionW(req, INTERNET_OPTION_URL, std::ptr::null_mut(), &mut len);
        if len == 0 {
            InternetCloseHandle(req);
            InternetCloseHandle(session);
            return Err(win_err("InternetQueryOption(URL)"));
        }
        let mut buf = vec![0u16; (len as usize / 2) + 2];
        let mut len2 = (buf.len() * 2) as u32;
        let ok = InternetQueryOptionW(
            req,
            INTERNET_OPTION_URL,
            buf.as_mut_ptr() as *mut _,
            &mut len2,
        );
        InternetCloseHandle(req);
        InternetCloseHandle(session);
        if ok == 0 {
            return Err(win_err("InternetQueryOption(URL)"));
        }
        let final_url = String::from_utf16_lossy(&buf);
        let tag = final_url.split("/tag/").nth(1).unwrap_or("").trim().to_string();
        let cleaned: String = tag
            .trim_start_matches('v')
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect();
        if parse_ver(&cleaned).is_none() {
            return Err(format!("неожиданный адрес: {final_url}"));
        }
        Ok(cleaned)
    }
}

#[cfg(windows)]
fn win_err(stage: &str) -> String {
    format!("{}: {}", stage, std::io::Error::last_os_error())
}

#[cfg(not(windows))]
pub fn check_latest() -> CheckResult {
    Err("нет WinINet".to_string())
}

/// Отправить форму POST-запросом через системный WinINet.
///
/// Нужен для кнопки «Отправить» в отчёте о проблеме: данные уходят в
/// Google-форму напрямую, без браузера и без окна консоли. Возвращает код
/// ответа HTTP (200/302 — форма приняла ответ).
#[cfg(windows)]
pub fn post_form(host: &str, path: &str, body: &str) -> Result<u32, String> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "wininet", kind = "raw-dylib")]
    extern "system" {
        fn InternetOpenW(
            agent: *const u16,
            access: u32,
            proxy: *const u16,
            bypass: *const u16,
            flags: u32,
        ) -> *mut core::ffi::c_void;
        fn InternetConnectW(
            session: *mut core::ffi::c_void,
            server: *const u16,
            port: u16,
            user: *const u16,
            pass: *const u16,
            service: u32,
            flags: u32,
            context: usize,
        ) -> *mut core::ffi::c_void;
        fn HttpOpenRequestW(
            conn: *mut core::ffi::c_void,
            verb: *const u16,
            object: *const u16,
            version: *const u16,
            referrer: *const u16,
            accept: *const *const u16,
            flags: u32,
            context: usize,
        ) -> *mut core::ffi::c_void;
        fn HttpSendRequestW(
            req: *mut core::ffi::c_void,
            headers: *const u16,
            headers_len: u32,
            body: *mut core::ffi::c_void,
            body_len: u32,
        ) -> i32;
        fn HttpQueryInfoW(
            req: *mut core::ffi::c_void,
            level: u32,
            buffer: *mut core::ffi::c_void,
            len: *mut u32,
            index: *mut u32,
        ) -> i32;
        fn InternetSetOptionW(
            h: *mut core::ffi::c_void,
            option: u32,
            buffer: *const core::ffi::c_void,
            len: u32,
        ) -> i32;
        fn InternetCloseHandle(h: *mut core::ffi::c_void) -> i32;
    }

    const INTERNET_OPEN_TYPE_PRECONFIG: u32 = 0;
    const INTERNET_SERVICE_HTTP: u32 = 3;
    const INTERNET_OPTION_CONNECT_TIMEOUT: u32 = 2;
    const INTERNET_OPTION_SEND_TIMEOUT: u32 = 5;
    const INTERNET_OPTION_RECEIVE_TIMEOUT: u32 = 6;
    const INTERNET_FLAG_SECURE: u32 = 0x0080_0000;
    const INTERNET_FLAG_RELOAD: u32 = 0x8000_0000;
    const INTERNET_FLAG_NO_CACHE_WRITE: u32 = 0x0400_0000;
    const INTERNET_FLAG_NO_UI: u32 = 0x0000_0200;
    const HTTP_QUERY_STATUS_CODE: u32 = 19;
    const HTTP_QUERY_FLAG_NUMBER: u32 = 0x2000_0000;

    let wide = |s: &str| -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };

    unsafe {
        let agent = wide("ShatMat");
        let session = InternetOpenW(
            agent.as_ptr(),
            INTERNET_OPEN_TYPE_PRECONFIG,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if session.is_null() {
            return Err(win_err("InternetOpen"));
        }
        let timeout: u32 = 12000;
        for option in [
            INTERNET_OPTION_CONNECT_TIMEOUT,
            INTERNET_OPTION_SEND_TIMEOUT,
            INTERNET_OPTION_RECEIVE_TIMEOUT,
        ] {
            InternetSetOptionW(
                session,
                option,
                &timeout as *const u32 as *const _,
                4,
            );
        }
        let host_w = wide(host);
        let conn = InternetConnectW(
            session,
            host_w.as_ptr(),
            443,
            std::ptr::null(),
            std::ptr::null(),
            INTERNET_SERVICE_HTTP,
            0,
            0,
        );
        if conn.is_null() {
            let e = win_err("InternetConnect");
            InternetCloseHandle(session);
            return Err(e);
        }
        let verb = wide("POST");
        let object = wide(path);
        let req = HttpOpenRequestW(
            conn,
            verb.as_ptr(),
            object.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            INTERNET_FLAG_SECURE
                | INTERNET_FLAG_RELOAD
                | INTERNET_FLAG_NO_CACHE_WRITE
                | INTERNET_FLAG_NO_UI,
            0,
        );
        if req.is_null() {
            let e = win_err("HttpOpenRequest");
            InternetCloseHandle(conn);
            InternetCloseHandle(session);
            return Err(e);
        }
        let headers = wide("Content-Type: application/x-www-form-urlencoded\r\n");
        let mut body_bytes = body.as_bytes().to_vec();
        let sent = HttpSendRequestW(
            req,
            headers.as_ptr(),
            u32::MAX,
            body_bytes.as_mut_ptr() as *mut core::ffi::c_void,
            body_bytes.len() as u32,
        );
        if sent == 0 {
            let e = win_err("HttpSendRequest");
            InternetCloseHandle(req);
            InternetCloseHandle(conn);
            InternetCloseHandle(session);
            return Err(e);
        }
        let mut status: u32 = 0;
        let mut len: u32 = 4;
        let mut index: u32 = 0;
        let ok = HttpQueryInfoW(
            req,
            HTTP_QUERY_STATUS_CODE | HTTP_QUERY_FLAG_NUMBER,
            &mut status as *mut u32 as *mut _,
            &mut len,
            &mut index,
        );
        InternetCloseHandle(req);
        InternetCloseHandle(conn);
        InternetCloseHandle(session);
        if ok == 0 {
            return Err(win_err("HttpQueryInfo"));
        }
        Ok(status)
    }
}

#[cfg(not(windows))]
pub fn post_form(_host: &str, _path: &str, _body: &str) -> Result<u32, String> {
    Err("нет WinINet".to_string())
}

/// Открыть произвольную ссылку в браузере (ShellExecuteW, без cmd).
#[cfg(windows)]
pub fn open_url(url: &str) {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut core::ffi::c_void,
            op: *const u16,
            file: *const u16,
            params: *const u16,
            dir: *const u16,
            show: i32,
        ) -> isize;
    }
    let wide = |s: &str| -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };
    let op = wide("open");
    let file = wide(url);
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            op.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        );
    }
}

#[cfg(not(windows))]
pub fn open_url(_url: &str) {}

/// Открыть страницу релизов в браузере (ShellExecuteW, без cmd).
#[cfg(windows)]
pub fn open_downloads() {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "shell32")]
    extern "system" {
        fn ShellExecuteW(
            hwnd: *mut core::ffi::c_void,
            op: *const u16,
            file: *const u16,
            params: *const u16,
            dir: *const u16,
            show: i32,
        ) -> isize;
    }
    let wide = |s: &str| -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };
    let op = wide("open");
    let file = wide(RELEASE_URL);
    unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            op.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
        );
    }
}

#[cfg(not(windows))]
pub fn open_downloads() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_compare() {
        assert!(is_newer("v0.18.1", "0.18.0"));
        assert!(is_newer("0.19", "0.18.9"));
        assert!(is_newer("v1.0", "0.9.9"));
        assert!(!is_newer("v0.18.0", "0.18.0"));
        assert!(!is_newer("v0.17.9", "0.18.0"));
        assert!(!is_newer("", "0.18.0"));
        assert!(!is_newer("latest", "0.18.0"));
    }
}
