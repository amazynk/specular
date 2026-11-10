//! Shell thumbnails: how file managers get Specular's previews of the formats it decodes.
//!
//! The shared part is the command `specular --thumbnail IN OUT SIZE` (`app/specular/src/
//! thumbnail.rs`): it opens IN through the decoder registry, decodes at the coarsest scale
//! still at least SIZE pixels on the long side (or uses the embedded preview), applies the
//! orientation, scales down and writes a PNG with the freedesktop `Thumb::*` text chunks.
//! Each desktop wires it in its own way:
//!
//! # Linux and BSD (done)
//!
//! `packaging/linux/specular.thumbnailer`, installed to `/usr/share/thumbnailers/`. The
//! file manager (GNOME's `gnome-desktop` thumbnail factory, tumbler, KDE via
//! `kio-extras`' desktop-thumbnailer support) runs `specular --thumbnail %i %o %s` in its
//! own sandbox (bubblewrap on GNOME) and caches the PNG under
//! `$XDG_CACHE_HOME/thumbnails/{normal,large,x-large,xx-large}/`. Nothing in this crate is
//! needed.
//!
//! # Windows: `IThumbnailProvider` (design stub, not built)
//!
//! Explorer loads thumbnail handlers as in-process COM servers (inside `dllhost.exe`
//! surrogates since Windows 7 when `DisableProcessIsolation` is not set), so this is a
//! separate `cdylib` (`specular-thumb.dll`), not the viewer binary:
//!
//! * A COM class implementing `IInitializeWithStream` (preferred: Explorer hands over an
//!   `IStream`, which also works for files inside archives and on cloud placeholders) and
//!   `IThumbnailProvider::GetThumbnail(cx, *mut HBITMAP, *mut WTS_ALPHATYPE)`.
//! * `GetThumbnail` reads the stream into memory (bounded: refuse streams above a size cap),
//!   runs the same registry path as `--thumbnail` with `SIZE = cx`, and returns a top-down
//!   32-bit premultiplied BGRA DIB section with `WTSAT_ARGB` (`WTSAT_RGB` for opaque images).
//! * Registration (per user, no admin rights needed, done by the installer):
//!   `HKCU\Software\Classes\CLSID\{clsid}\InprocServer32` = path of the DLL,
//!   `ThreadingModel = Apartment`; and for each extension
//!   `HKCU\Software\Classes\.jxl\ShellEx\{e357fccd-a995-4576-b01f-234630154e96}` = `{clsid}`.
//!   Extensions Windows already thumbnails well (`.jpg`, `.png`) are left alone.
//! * `DllGetClassObject` / `DllCanUnloadNow` exports; the `windows` crate's `implement`
//!   macro generates the vtables. The COM boundary is the one place this needs `unsafe`
//!   (raw interface pointers, `HBITMAP` ownership), kept in a small module listed in
//!   UNSAFE.md.
//! * Failure must be cheap and quiet: return `E_FAIL` and Explorer shows the generic icon.
//!   Decoding runs on Explorer's thread with its time budget, so the handler decodes at
//!   reduced scale only and never uses the GPU.
//!
//! # macOS: Quick Look (design stub, not built)
//!
//! Finder thumbnails and Space-bar previews come from app extensions in the app bundle,
//! built with Xcode tooling around a small Rust static library:
//!
//! * `SpecularThumbnail.appex`, a `QLThumbnailProvider` subclass (Quick Look Thumbnailing,
//!   macOS 10.15+). `provideThumbnail(for:_:)` calls a C ABI function from the Rust
//!   library, `specular_thumbnail(path, max_px, out_rgba, out_w, out_h) -> i32`, which runs
//!   the `--thumbnail` pipeline, and draws the pixels into the `QLThumbnailReply` context
//!   (`CGImage` from the RGBA buffer, sized to `request.maximumSize * request.scale`).
//! * `SpecularPreview.appex`, a `QLPreviewProvider` (macOS 12+) returning a
//!   `QLPreviewReply` with image data for the Space-bar preview of formats macOS cannot
//!   show (JPEG XL before macOS 14, EXR, Radiance HDR, PSD layers flattened, camera raw
//!   variants), using a larger size.
//! * `Info.plist` of each extension: `NSExtensionPointIdentifier`
//!   `com.apple.quicklook.thumbnail` / `com.apple.quicklook.preview`, and `QLSupportedContentTypes`
//!   with the UTIs (`public.jpeg-xl`, `com.ilm.openexr-image`, `public.avif`, …), declared
//!   in the app's `UTImportedTypeDeclarations` where the system has none.
//! * Extensions are sandboxed: they read only the file Quick Look hands them (security
//!   scoped), which the registry path already works with (it reads one file, no siblings).
//!   No GPU; reduced-scale decodes only.
//!
//! # Shared rules
//!
//! * The same output everywhere: orientation applied, straight alpha in the PNG (Windows
//!   wants premultiplied BGRA, converted at the boundary).
//! * Bounded work: decode at the scale the size needs; refuse inputs whose header promises
//!   more pixels than the memory budget for a thumbnail allows.
//! * No settings, no log file, no window: thumbnailers run in other processes' sandboxes.

/// What a thumbnail request asks for: the long side in pixels, as the shell gives it
/// (`%s` on Linux, `cx` on Windows, `maximumSize × scale` on macOS).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThumbnailSize(pub u32);

impl ThumbnailSize {
    /// The freedesktop cache sizes (`normal`, `large`, `x-large`, `xx-large`).
    pub const FREEDESKTOP: [u32; 4] = [128, 256, 512, 1024];

    /// Requests above this are clamped: no shell asks for more, and it bounds the work.
    pub const MAX: u32 = 4096;

    pub fn clamped(self) -> u32 {
        self.0.clamp(1, Self::MAX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps() {
        assert_eq!(ThumbnailSize(0).clamped(), 1);
        assert_eq!(ThumbnailSize(256).clamped(), 256);
        assert_eq!(ThumbnailSize(100_000).clamped(), ThumbnailSize::MAX);
    }
}
