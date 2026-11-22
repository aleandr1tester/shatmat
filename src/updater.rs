//! Проверка обновлений: страница последнего релиза на GitHub.
//!
//! Запрос идёт через системный WinHTTP — никаких новых крейтов и внешних
//! процессов: для антивируса это обычный HTTPS-запрос стандартной библиотеки
//! Windows. Игра ничего не скачивает и не заменяет: по клику открывается
//! страница релиза в браузере, установку делает сам пользователь.

/// Версия этой сборки («0.18.0»).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const RELEASE_URL: &str = "https://github.com/aleandr1tester/shatmat/releases/latest";

/// Результат фоновой проверки: версия последнего релиза («0.18.1») или ошибка.
pub type CheckResult = Result<String, ()>;

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
/// Хитрость без разбора JSON: «…/releases/latest» редиректит на
/// «…/releases/tag/vX.Y.Z», а WinHTTP умеет отдать итоговый адрес
/// после перехода (WINHTTP_OPTION_URL).
#[cfg(windows)]
pub fn check_latest() -> CheckResult {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "winhttp")]
    extern "system" {
        fn WinHttpOpen(
            agent: *const u16,
            access: u32,
            proxy: *const u16,
            bypass: *const u16,
            flags: u32,
        ) -> *mut core::ffi::c_void;
        fn WinHttpSetTimeouts(
            h: *mut core::ffi::c_void,
            resolve: i32,
            connect: i32,
            send: i32,
            receive: i32,
        ) -> i32;
        fn WinHttpConnect(
            h: *mut core::ffi::c_void,
            host: *const u16,
            port: u16,
            reserved: u32,
        ) -> *mut core::ffi::c_void;
        fn WinHttpOpenRequest(
            h: *mut core::ffi::c_void,
            verb: *const u16,
            object: *const u16,
            version: *const u16,
            referrer: *const u16,
            accept: *const *const u16,
            flags: u32,
        ) -> *mut core::ffi::c_void;
        fn WinHttpSendRequest(
            h: *mut core::ffi::c_void,
            headers: *const u16,
            len: u32,
            optional: *const core::ffi::c_void,
            optlen: u32,
            total: u32,
            context: usize,
        ) -> i32;
        fn WinHttpReceiveResponse(h: *mut core::ffi::c_void, reserved: *mut core::ffi::c_void) -> i32;
        fn WinHttpQueryOption(
            h: *mut core::ffi::c_void,
            option: u32,
            buffer: *mut core::ffi::c_void,
            len: *mut u32,
        ) -> i32;
        fn WinHttpCloseHandle(h: *mut core::ffi::c_void) -> i32;
    }

    const WINHTTP_FLAG_SECURE: u32 = 0x0080_0000;
    const WINHTTP_OPTION_URL: u32 = 93;

    let wide = |s: &str| -> Vec<u16> {
        std::ffi::OsStr::new(s)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    };

    unsafe {
        let agent = wide("ShatMat");
        // Доступ по умолчанию: системный прокси пользователя.
        let session = WinHttpOpen(agent.as_ptr(), 0, std::ptr::null(), std::ptr::null(), 0);
        if session.is_null() {
            return Err(());
        }
        WinHttpSetTimeouts(session, 4000, 4000, 4000, 4000);
        let host = wide("github.com");
        let connect = WinHttpConnect(session, host.as_ptr(), 443, 0);
        if connect.is_null() {
            WinHttpCloseHandle(session);
            return Err(());
        }
        let verb = wide("GET");
        let object = wide("/aleandr1tester/shatmat/releases/latest");
        let request = WinHttpOpenRequest(
            connect,
            verb.as_ptr(),
            object.as_ptr(),
            std::ptr::null(), // HTTP/1.1 по умолчанию
            std::ptr::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        );
        if request.is_null() {
            WinHttpCloseHandle(connect);
            WinHttpCloseHandle(session);
            return Err(());
        }
        let headers = wide("User-Agent: ShatMat\r\n");
        let sent = WinHttpSendRequest(
            request,
            headers.as_ptr(),
            (headers.len() as u32 - 1) * 2,
            std::ptr::null(),
            0,
            0,
            0,
        );
        let received = if sent != 0 {
            WinHttpReceiveResponse(request, std::ptr::null_mut())
        } else {
            0
        };
        let mut len: u32 = 0;
        let mut url = String::new();
        if sent != 0 && received != 0 {
            WinHttpQueryOption(request, WINHTTP_OPTION_URL, std::ptr::null_mut(), &mut len);
            if len > 0 {
                let mut buf = vec![0u16; len as usize / 2];
                if WinHttpQueryOption(
                    request,
                    WINHTTP_OPTION_URL,
                    buf.as_mut_ptr() as *mut _,
                    &mut len,
                ) != 0
                {
                    url = String::from_utf16_lossy(&buf);
                }
            }
        }
        WinHttpCloseHandle(request);
        WinHttpCloseHandle(connect);
        WinHttpCloseHandle(session);
        // …/releases/tag/v0.18.1 → «0.18.1»
        url.split("/tag/")
            .nth(1)
            .map(|t| t.trim().trim_start_matches('v').trim().to_string())
            .filter(|t| !t.is_empty() && parse_ver(t).is_some())
            .ok_or(())
    }
}

#[cfg(not(windows))]
pub fn check_latest() -> CheckResult {
    Err(())
}

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
