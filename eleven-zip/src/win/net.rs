//! HTTPS requests through WinHTTP, which uses Windows' own TLS, certificate
//! store and proxy settings.

use std::ffi::c_void;

use windows::Win32::Networking::WinHttp::{
    INTERNET_DEFAULT_HTTPS_PORT, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_ADDREQ_FLAG_ADD,
    WINHTTP_FLAG_SECURE, WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE,
    WinHttpAddRequestHeaders, WinHttpCloseHandle, WinHttpConnect, WinHttpOpen, WinHttpOpenRequest,
    WinHttpQueryHeaders, WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest,
    WinHttpSetTimeouts,
};
use windows::core::{HSTRING, PCWSTR, w};

use crate::release::split_https;

/// Larger than anything eleven-zip downloads: a 7-Zip package is about 2 MB.
const LIMIT: usize = 64 * 1024 * 1024;
/// Milliseconds for each step: name resolution, connecting, sending, and
/// each wait for data.
const TIMEOUT: i32 = 30_000;

/// Closes a WinHTTP handle when dropped.
struct Handle(*mut c_void);

impl Handle {
    fn new(raw: *mut c_void, what: &str) -> Result<Handle, String> {
        if raw.is_null() {
            Err(format!("{what}: {}", windows::core::Error::from_thread()))
        } else {
            Ok(Handle(raw))
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        let _ = unsafe { WinHttpCloseHandle(self.0) };
    }
}

/// GETs `url` and returns the body of a 200 response. `accept` is sent as
/// the Accept header. Redirects are followed.
pub fn get(url: &str, accept: Option<&str>) -> Result<Vec<u8>, String> {
    let fail = |e: windows::core::Error| format!("通信に失敗しました: {e}");
    let (host, path) =
        split_https(url).ok_or_else(|| format!("HTTPS の URL ではありません: {url}"))?;
    let agent = HSTRING::from(format!("eleven-zip/{}", env!("CARGO_PKG_VERSION")));
    let session = Handle::new(
        unsafe {
            WinHttpOpen(
                &agent,
                WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
                PCWSTR::null(),
                PCWSTR::null(),
                0,
            )
        },
        "WinHttpOpen",
    )?;
    unsafe { WinHttpSetTimeouts(session.0, TIMEOUT, TIMEOUT, TIMEOUT, TIMEOUT) }.map_err(fail)?;
    let connection = Handle::new(
        unsafe {
            WinHttpConnect(
                session.0,
                &HSTRING::from(host),
                INTERNET_DEFAULT_HTTPS_PORT,
                0,
            )
        },
        "WinHttpConnect",
    )?;
    let request = Handle::new(
        unsafe {
            WinHttpOpenRequest(
                connection.0,
                w!("GET"),
                &HSTRING::from(path),
                PCWSTR::null(),
                PCWSTR::null(),
                std::ptr::null(),
                WINHTTP_FLAG_SECURE,
            )
        },
        "WinHttpOpenRequest",
    )?;
    if let Some(accept) = accept {
        let header: Vec<u16> = format!("Accept: {accept}").encode_utf16().collect();
        unsafe { WinHttpAddRequestHeaders(request.0, &header, WINHTTP_ADDREQ_FLAG_ADD) }
            .map_err(fail)?;
    }
    unsafe {
        WinHttpSendRequest(request.0, None, None, 0, 0, 0).map_err(fail)?;
        WinHttpReceiveResponse(request.0, std::ptr::null_mut()).map_err(fail)?;
    }

    let mut status = 0u32;
    let mut size = size_of::<u32>() as u32;
    unsafe {
        WinHttpQueryHeaders(
            request.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut size,
            std::ptr::null_mut(),
        )
    }
    .map_err(fail)?;
    if status != 200 {
        return Err(format!("{host} が {status} を返しました"));
    }

    let mut body = Vec::new();
    let mut chunk = vec![0u8; 64 * 1024];
    loop {
        let mut read = 0u32;
        unsafe {
            WinHttpReadData(
                request.0,
                chunk.as_mut_ptr().cast(),
                chunk.len() as u32,
                &mut read,
            )
        }
        .map_err(fail)?;
        if read == 0 {
            return Ok(body);
        }
        body.extend_from_slice(&chunk[..read as usize]);
        if body.len() > LIMIT {
            return Err(format!("{host} からの応答が大きすぎます"));
        }
    }
}
