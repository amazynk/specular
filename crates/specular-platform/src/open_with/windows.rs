//! "Open with…" on Windows: `SHAssocEnumHandlers` lists the recommended handlers for the file's
//! extension; `IAssocHandler::Invoke` opens the file with one (through the shell, so packaged
//! apps and per-user registrations work). Each `unsafe` is listed in UNSAFE.md.

use std::io;
use std::path::Path;

use windows::Win32::System::Com::{
    COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize, IDataObject,
};
use windows::Win32::UI::Shell::{
    ASSOC_FILTER_RECOMMENDED, BHID_DataObject, IAssocHandler, IShellItem, SHAssocEnumHandlers,
    SHCreateItemFromParsingName,
};
use windows::core::{HSTRING, PWSTR};

use super::{Handler, How};

/// COM for the current thread for as long as this lives.
struct Com(bool);

impl Com {
    #[allow(unsafe_code)]
    fn init() -> Com {
        // SAFETY: plain COM initialisation of the calling thread; balanced by `Drop` only when
        // it succeeded (S_OK or S_FALSE).
        let hr = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        Com(hr.is_ok())
    }
}

impl Drop for Com {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the successful `CoInitializeEx` on this same thread.
            unsafe { CoUninitialize() };
        }
    }
}

/// Takes a shell-allocated string.
#[allow(unsafe_code)]
fn take(p: PWSTR) -> String {
    if p.is_null() {
        return String::new();
    }
    // SAFETY: the shell returned a valid NUL-terminated wide string it allocated with
    // CoTaskMemAlloc; it is read once and freed once.
    let s = unsafe { p.to_string() }.unwrap_or_default();
    // SAFETY: see above; the pointer is not used afterwards.
    unsafe { CoTaskMemFree(Some(p.0 as *const _)) };
    s
}

/// Every recommended handler for `path`'s extension, as COM objects.
#[allow(unsafe_code)]
fn enumerate(path: &Path) -> Vec<IAssocHandler> {
    let Some(ext) = path.extension() else {
        return Vec::new();
    };
    let ext = HSTRING::from(format!(".{}", ext.to_string_lossy()));
    // SAFETY: COM is initialised on this thread (callers hold a `Com`); `ext` outlives the call.
    let Ok(en) = (unsafe { SHAssocEnumHandlers(&ext, ASSOC_FILTER_RECOMMENDED) }) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    loop {
        let mut one = [None];
        let mut got = 0u32;
        // SAFETY: `one` has room for the one element asked for; `got` is a valid out pointer.
        let r = unsafe { en.Next(&mut one, Some(&mut got)) };
        match one[0].take() {
            Some(h) if r.is_ok() && got == 1 => out.push(h),
            _ => break,
        }
        if out.len() > 64 {
            break;
        }
    }
    out
}

#[allow(unsafe_code)]
pub(super) fn handlers_for(path: &Path) -> Vec<Handler> {
    let _com = Com::init();
    let mut out = Vec::new();
    for h in enumerate(path) {
        // SAFETY: `h` is a live handler from the enumeration; the strings are taken and freed
        // by `take`.
        let id = unsafe { h.GetName() }.map(take).unwrap_or_default();
        // SAFETY: as above.
        let name = unsafe { h.GetUIName() }.map(take).unwrap_or_default();
        if id.is_empty() || name.is_empty() || is_specular(&id) {
            continue;
        }
        out.push(Handler {
            name,
            id,
            is_default: false,
            how: How::Assoc,
        });
    }
    // The shell lists the default first.
    if let Some(first) = out.first_mut() {
        first.is_default = true;
    }
    out
}

fn is_specular(id: &str) -> bool {
    Path::new(id)
        .file_stem()
        .is_some_and(|s| s.eq_ignore_ascii_case("specular"))
}

#[allow(unsafe_code)]
pub(super) fn launch(id: &str, path: &Path) -> io::Result<()> {
    let _com = Com::init();
    let h = enumerate(path)
        .into_iter()
        // SAFETY: `h` is a live handler from the enumeration; `take` frees the string.
        .find(|h| unsafe { h.GetName() }.map(take).is_ok_and(|n| n == id))
        .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    let p = HSTRING::from(path.as_os_str());
    // SAFETY: COM is initialised; `p` outlives the call; no bind context.
    let item: IShellItem =
        unsafe { SHCreateItemFromParsingName(&p, None) }.map_err(io::Error::other)?;
    // SAFETY: `item` is live; BHID_DataObject is a static GUID.
    let data: IDataObject =
        unsafe { item.BindToHandler(None, &BHID_DataObject) }.map_err(io::Error::other)?;
    // SAFETY: `h` and `data` are live COM objects on this thread.
    unsafe { h.Invoke(&data) }.map_err(io::Error::other)
}
