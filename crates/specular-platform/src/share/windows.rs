//! Windows: the Share flyout (`DataTransferManager` through `IDataTransferManagerInterop`
//! for a desktop window) and the desktop picture (`IDesktopWallpaper`, falling back to
//! `SystemParametersInfoW(SPI_SETDESKWALLPAPER)`). Each `unsafe` is a COM or Win32 call with
//! the reasoning beside it.

use std::io;
use std::path::{Path, PathBuf};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::ApplicationModel::DataTransfer::{DataRequestedEventArgs, DataTransferManager};
use windows::Foundation::TypedEventHandler;
use windows::Storage::{IStorageItem, StorageFile};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{
    CLSCTX_ALL, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx, CoUninitialize,
};
use windows::Win32::UI::Shell::{DesktopWallpaper, IDataTransferManagerInterop, IDesktopWallpaper};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, SPI_SETDESKWALLPAPER, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE,
    SystemParametersInfoW,
};
use windows::core::{AgileReference, HSTRING, Interface, PCWSTR};
use windows_collections::IIterable;

fn win(e: windows::core::Error) -> io::Error {
    io::Error::other(e.message())
}

/// COM for the current thread for as long as this lives (also used by the recent-files
/// option, `integration::windows::recent`).
pub(crate) struct Com(bool);

impl Com {
    #[allow(unsafe_code)]
    pub(crate) fn init() -> Com {
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

fn hwnd_of(window: &dyn HasWindowHandle) -> io::Result<HWND> {
    match window
        .window_handle()
        .map_err(|e| io::Error::other(format!("{e:?}")))?
        .as_raw()
    {
        RawWindowHandle::Win32(h) => Ok(HWND(h.hwnd.get() as *mut _)),
        _ => Err(io::ErrorKind::Unsupported.into()),
    }
}

/// The Share flyout for `files` over `window`.
#[allow(unsafe_code)]
pub(crate) fn share(files: &[PathBuf], window: &dyn HasWindowHandle) -> io::Result<()> {
    let hwnd = hwnd_of(window)?;
    let mut items: Vec<AgileReference<IStorageItem>> = Vec::new();
    for f in files {
        let file = StorageFile::GetFileFromPathAsync(&HSTRING::from(f.as_os_str()))
            .and_then(|op| op.join())
            .map_err(win)?;
        // Agile references: the handler may run on another thread of the apartment.
        let item = file.cast::<IStorageItem>().map_err(win)?;
        items.push(AgileReference::new(&item).map_err(win)?);
    }
    let title = files
        .first()
        .and_then(|f| f.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let interop = windows::core::factory::<DataTransferManager, IDataTransferManagerInterop>()
        .map_err(win)?;
    // SAFETY: `hwnd` is this process's live top-level window (from winit); the call returns
    // the manager for it, owned as a COM reference.
    let manager: DataTransferManager = unsafe { interop.GetForWindow(hwnd) }.map_err(win)?;
    manager
        .DataRequested(&TypedEventHandler::<
            DataTransferManager,
            DataRequestedEventArgs,
        >::new(move |_, args| {
            let Some(args) = args.as_ref() else {
                return Ok(());
            };
            let data = args.Request()?.Data()?;
            data.Properties()?
                .SetTitle(&HSTRING::from(title.as_str()))?;
            let list: Vec<Option<IStorageItem>> = items
                .iter()
                .map(|a| a.resolve().map(Some))
                .collect::<windows::core::Result<_>>()?;
            let list: IIterable<IStorageItem> = list.into();
            data.SetStorageItemsReadOnly(&list)?;
            Ok(())
        }))
        .map_err(win)?;
    // SAFETY: as above; shows the system flyout for the same window.
    unsafe { interop.ShowShareUIForWindow(hwnd) }.map_err(win)
}

/// Sets `image` as the desktop picture of every monitor.
#[allow(unsafe_code)]
pub(crate) fn set_wallpaper(image: &Path) -> io::Result<&'static str> {
    let path = HSTRING::from(std::path::absolute(image)?.as_os_str());
    let _com = Com::init();
    // SAFETY: COM is initialised on this thread; the CLSID and interface are the shell's
    // documented desktop wallpaper object.
    let desk: windows::core::Result<IDesktopWallpaper> =
        unsafe { CoCreateInstance(&DesktopWallpaper, None, CLSCTX_ALL) };
    if let Ok(d) = desk {
        // SAFETY: a null monitor id means every monitor; `path` outlives the call.
        if unsafe { d.SetWallpaper(PCWSTR::null(), &path) }.is_ok() {
            return Ok("IDesktopWallpaper");
        }
    }
    // SAFETY: SPI_SETDESKWALLPAPER reads the NUL-terminated wide path `path` points to,
    // which lives until the call returns.
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(path.as_ptr() as *mut _),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    }
    .map_err(win)?;
    Ok("SystemParametersInfo")
}

/// Lets whichever process this one hands work to next bring its window to the front.
#[allow(unsafe_code)]
pub(crate) fn allow_foreground_handoff() {
    // SAFETY: takes a process id (here every process) and touches no memory. It fails when
    // this process is not in front, and then there is nothing to hand on.
    let _ = unsafe { AllowSetForegroundWindow(ASFW_ANY) };
}
