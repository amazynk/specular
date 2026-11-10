//! Why something failed, worded for the user: what happened and, where we know it, what to
//! do. Shown after "Can't open FILE:" and in the other "Couldn't …: REASON" messages, so
//! each reason is a lower-case phrase. Never Rust's debug text, and never "(os error 2)".

use specular_decode::DecodeError;
use specular_i18n::t;
use std::io::ErrorKind;

/// An I/O error's reason: the common kinds in the user's language, anything else as the
/// system words it, without the error number.
pub(crate) fn io_reason(e: &std::io::Error) -> String {
    // A remote store's failure, kept inside the I/O error by the Zarr reader.
    if let Some(k) = e
        .get_ref()
        .and_then(|i| i.downcast_ref::<specular_core::keys::KeyError>())
    {
        return key_reason(k);
    }
    if specular_core::net::off::is_refusal(e) {
        return t!("error-reason-network-off");
    }
    match e.kind() {
        ErrorKind::NotFound => t!("error-reason-not-found"),
        ErrorKind::PermissionDenied => t!("error-reason-permission"),
        ErrorKind::StorageFull | ErrorKind::QuotaExceeded => t!("error-reason-disk-full"),
        ErrorKind::ReadOnlyFilesystem => t!("error-reason-read-only"),
        ErrorKind::UnexpectedEof => t!("error-reason-truncated"),
        ErrorKind::IsADirectory => t!("error-reason-is-folder"),
        ErrorKind::TimedOut => t!("error-reason-timed-out"),
        ErrorKind::OutOfMemory => t!("error-reason-memory"),
        _ => system_words(&e.to_string()),
    }
}

/// Why reading a remote store (a Zarr image on a server or in a bucket) failed.
pub(crate) fn key_reason(e: &specular_core::keys::KeyError) -> String {
    use specular_core::keys::KeyError as K;
    match e {
        K::Cancelled => t!("error-reason-cancelled"),
        K::Denied => t!("error-reason-remote-denied"),
        K::Status(404 | 410) => t!("error-reason-remote-missing"),
        K::Status(s) => t!("error-reason-remote-status", status = i64::from(*s)),
        K::Offline(_) => t!("error-reason-offline"),
        K::Timeout => t!("error-reason-timed-out"),
        K::Failed(m) => text_reason(m),
        K::NetworkOff => t!("error-reason-network-off"),
    }
}

/// Why a program did not start: "not found" here means the program, not the image.
pub(crate) fn spawn_reason(e: &std::io::Error) -> String {
    match e.kind() {
        ErrorKind::NotFound => t!("error-reason-program-missing"),
        ErrorKind::PermissionDenied => t!("error-reason-program-not-allowed"),
        _ => io_reason(e),
    }
}

/// Why moving to or restoring from the trash failed.
pub(crate) fn trash_reason(e: &trash::Error) -> String {
    match e {
        #[cfg(all(
            unix,
            not(target_os = "macos"),
            not(target_os = "ios"),
            not(target_os = "android")
        ))]
        trash::Error::FileSystem { source, .. } => io_reason(source),
        trash::Error::CouldNotAccess { .. } | trash::Error::CanonicalizePath { .. } => {
            t!("error-reason-trash-access")
        }
        trash::Error::RestoreCollision { .. } | trash::Error::RestoreTwins { .. } => {
            t!("error-reason-restore-collision")
        }
        trash::Error::TargetedRoot => t!("error-reason-trash-root"),
        trash::Error::Os { description, .. } | trash::Error::Unknown { description } => {
            system_words(description)
        }
        other => system_words(&other.to_string()),
    }
}

/// A decoder's reason for not opening a file.
pub(crate) fn decode_reason(e: &DecodeError) -> String {
    match e {
        DecodeError::UnsupportedFormat => t!("error-reason-unsupported"),
        DecodeError::Ambiguous(_) => t!("error-reason-ambiguous"),
        DecodeError::NotOneOf { ext, formats } => t!(
            "error-reason-not-one-of",
            ext = *ext,
            formats = formats.join(", ")
        ),
        DecodeError::Unsupported(what) => {
            t!("error-reason-unsupported-detail", detail = detail(what))
        }
        DecodeError::Truncated(_) => t!("error-reason-truncated"),
        DecodeError::Malformed(what) => t!("error-reason-damaged", detail = detail(what)),
        DecodeError::TooLarge => t!("error-reason-too-large"),
        DecodeError::NeedsMemory(n) => {
            let (needed, free) = memory_amounts(*n);
            t!("error-reason-needs-memory", needed = needed, free = free)
        }
        DecodeError::NeedsDecoder(id) => t!(
            "error-reason-needs-decoder",
            format = id.format_name(),
            decoder = id.decoder_name(),
            vendor = id.vendor()
        ),
        DecodeError::Cancelled => t!("error-reason-cancelled"),
        DecodeError::Io(io) => io_reason(io),
        // The panic's message, location and crate ("internal error in specular-jpeg") are in
        // the log; the user is told in their language.
        DecodeError::Internal(_) => t!("error-reason-internal"),
    }
}

/// "This image needs 24 GB of free RAM, but only 18 GB is free right now. …": a gallery
/// tile's tooltip and the info panel.
pub(crate) fn needs_memory_text(n: specular_core::NeedsMemory) -> String {
    let (needed, free) = memory_amounts(n);
    t!("error-needs-memory", needed = needed, free = free)
}

/// "it needs 24 GB of free RAM, but only 18 GB is free right now. …": a reason within a
/// sentence (MPR's "Cannot build a volume: …").
pub(crate) fn needs_memory_reason(n: specular_core::NeedsMemory) -> String {
    let (needed, free) = memory_amounts(n);
    t!("error-reason-needs-memory", needed = needed, free = free)
}

/// "Needs 24 GB of free RAM": under a gallery tile.
pub(crate) fn needs_memory_short(n: specular_core::NeedsMemory) -> String {
    t!("error-needs-memory-short", needed = memory_amounts(n).0)
}

/// The amounts of a [`specular_core::NeedsMemory`] in the locale's words: what is needed
/// rounded up and what is free rounded down at the precision shown, so the two never read
/// the same ("needs 18 GB … only 18 GB is free").
fn memory_amounts(n: specular_core::NeedsMemory) -> (String, String) {
    let loc = specular_i18n::I18n::global().localizer();
    let half = |b: u64| shown_step(b) / 2;
    let needed = n.needed.saturating_add(half(n.needed).saturating_sub(1));
    let free = n.free.saturating_sub(half(n.free));
    (
        loc.bytes(needed, specular_i18n::ByteUnits::Decimal),
        loc.bytes(free, specular_i18n::ByteUnits::Decimal),
    )
}

/// The step `Localizer::bytes` rounds `b` to in decimal units: a tenth of the unit below 10
/// units, else the unit.
fn shown_step(b: u64) -> u64 {
    let mut unit = 1u64;
    while unit < 1_000_000_000_000 && b >= unit * 1000 {
        unit *= 1000;
    }
    if unit > 1 && b < unit * 10 {
        unit / 10
    } else {
        unit
    }
}

/// Why a video clip (a motion photo's) does not play.
pub(crate) fn play_reason(e: &specular_video::play::PlayError) -> String {
    use specular_video::play::PlayError as P;
    match e {
        P::Unsupported(what) => t!("error-reason-unsupported-detail", detail = detail(what)),
        P::Malformed(what) => t!("error-reason-damaged", detail = detail(what)),
        P::Io(what) => system_words(what),
        P::NeedsMemory(n) => {
            let (needed, free) = memory_amounts(*n);
            t!("error-reason-needs-memory", needed = needed, free = free)
        }
    }
}

/// Why a DICOM file did not read.
pub(crate) fn dicom_reason(e: &specular_dicom::Error) -> String {
    use specular_dicom::Error as E;
    match e {
        E::NotDicom => t!("error-reason-not-dicom"),
        E::Truncated => t!("error-reason-truncated"),
        E::Malformed(what) => t!("error-reason-damaged", detail = detail(what)),
        E::Unsupported(what) | E::Codec(what) => {
            t!("error-reason-unsupported-detail", detail = detail(what))
        }
        E::NoPixelData => t!("error-reason-no-image"),
        E::TooLarge(_) => t!("error-reason-too-large"),
        E::NeedsMemory(n) => {
            let (needed, free) = memory_amounts(*n);
            t!("error-reason-needs-memory", needed = needed, free = free)
        }
        E::Cancelled => t!("error-reason-cancelled"),
    }
}

/// Why a medical research volume (NIfTI, NRRD, MetaImage, …) did not read.
pub(crate) fn medvol_reason(e: &specular_decode::medvol::Error) -> String {
    use specular_decode::medvol::Error as E;
    match e {
        E::NotRecognised => t!("error-reason-unsupported"),
        E::Truncated(_) => t!("error-reason-truncated"),
        E::Malformed(what) => t!("error-reason-damaged", detail = detail(what)),
        E::Unsupported(what) => t!("error-reason-unsupported-detail", detail = detail(what)),
        E::Io(what) => system_words(&detail(what)),
        E::TooLarge => t!("error-reason-too-large"),
        E::NeedsLayout => t!("error-reason-needs-layout"),
        E::NeedsMemory(n) => {
            let (needed, free) = memory_amounts(*n);
            t!("error-reason-needs-memory", needed = needed, free = free)
        }
    }
}

/// Why an archive (or a comic book) or an entry of it did not open.
pub(crate) fn archive_reason(e: &specular_archive::Error) -> String {
    use specular_archive::Error as E;
    match e {
        E::Io(io) => io_reason(io),
        E::NotAnArchive => t!("error-reason-not-archive"),
        E::Malformed(what) => t!("error-reason-damaged", detail = detail(what)),
        E::Unsupported(what) => t!("error-reason-unsupported-detail", detail = detail(what)),
        E::NeedsSandbox(_) => t!("error-reason-needs-sandbox"),
        E::Encrypted => t!("error-reason-encrypted"),
        E::WrongPassword => t!("error-reason-wrong-password"),
        E::Limit(_) => t!("error-reason-too-large"),
        E::Checksum => t!("error-reason-checksum"),
        E::NoEntry(_) => t!("error-reason-missing-entry"),
    }
}

/// Why a series does not make a volume (the multiplanar view).
pub(crate) fn refusal_reason(r: &specular_dicom::volume::Refusal) -> String {
    use specular_dicom::volume::Refusal as R;
    match r {
        R::TooFew(n) => t!("error-reason-volume-too-few", count = *n as i64),
        R::NoGeometry => t!("error-reason-volume-no-geometry"),
        R::NotParallel => t!("error-reason-volume-not-parallel"),
        R::SharedPosition(n) => t!("error-reason-volume-shared-position", count = *n as i64),
        R::Missing(n) => t!("error-reason-volume-missing", count = i64::from(*n)),
        R::Mismatch => t!("error-reason-volume-mismatch"),
        R::NotGrey => t!("error-reason-volume-not-grey"),
        R::TooLarge { need, budget } => t!(
            "error-reason-volume-too-large",
            need = (*need >> 20) as i64,
            budget = (*budget >> 20) as i64
        ),
    }
}

/// Why a `specular:` link was refused. Its offsets and parameter names are for the log.
pub(crate) fn url_reason(e: &specular_platform::url_scheme::Error) -> String {
    use specular_platform::url_scheme::Error as E;
    match e {
        E::NotSpecular => t!("error-reason-url-not-specular"),
        E::TooLong => t!("error-reason-url-too-long"),
        E::BadCharacter(_)
        | E::BadEscape(_)
        | E::NotUtf8
        | E::Nul
        | E::Fragment
        | E::EmptyValue(_)
        | E::BadFileUri => t!("error-reason-url-damaged"),
        E::UnknownAction(_) | E::UnknownParameter(_) => t!("error-reason-url-unknown"),
        E::NotAbsolute => t!("error-reason-url-not-absolute"),
        E::NoPaths => t!("error-reason-url-no-paths"),
        E::TooManyPaths => t!("error-reason-url-too-many"),
    }
}

/// Why the Library's catalogue could not be opened, read or written.
pub(crate) fn library_reason(e: &specular_library::Error) -> String {
    match e.io() {
        Some(io) if e.class() == specular_library::ErrorClass::Io => io_reason(io),
        _ => library_class_reason(e.class(), None),
    }
}

/// [`library_reason`] from what is kept of an error (the indexer's [`specular_library::Halt`]).
pub(crate) fn library_class_reason(
    class: specular_library::ErrorClass,
    io: Option<std::io::ErrorKind>,
) -> String {
    use specular_library::ErrorClass as C;
    match class {
        C::Io => io.map_or_else(
            || t!("error-reason-internal"),
            |k| io_reason(&std::io::Error::from(k)),
        ),
        C::Busy => t!("error-reason-busy"),
        C::Corrupt => t!("error-reason-catalogue-damaged"),
        C::Full => t!("error-reason-disk-full"),
        C::Permission => t!("error-reason-permission"),
        C::ReadOnly => t!("error-reason-read-only"),
        C::CannotOpen => t!("error-reason-cannot-open"),
        C::Memory => t!("error-reason-memory"),
        C::Newer => t!("error-reason-newer-version"),
        C::Other => t!("error-reason-internal"),
    }
}

/// Any error: the first in its chain with a reason of its own (an I/O error, a decoder's,
/// a DICOM file's, an archive's, the Library's), else its own words, cut short and without
/// " (os error N)".
pub(crate) fn reason(e: &(dyn std::error::Error + 'static)) -> String {
    let mut at: Option<&(dyn std::error::Error + 'static)> = Some(e);
    while let Some(e) = at {
        if let Some(e) = e.downcast_ref::<std::io::Error>() {
            return io_reason(e);
        }
        if let Some(e) = e.downcast_ref::<specular_remote::Error>()
            && e.is_network_off()
        {
            return t!("error-reason-network-off");
        }
        if let Some(e) = e.downcast_ref::<DecodeError>() {
            return decode_reason(e);
        }
        if let Some(e) = e.downcast_ref::<specular_dicom::Error>() {
            return dicom_reason(e);
        }
        if let Some(e) = e.downcast_ref::<specular_archive::Error>() {
            return archive_reason(e);
        }
        if let Some(e) = e.downcast_ref::<specular_library::Error>() {
            return library_reason(e);
        }
        if let Some(e) = e.downcast_ref::<specular_decode::medvol::Error>() {
            return medvol_reason(e);
        }
        at = e.source();
    }
    text_reason(&e.to_string())
}

/// An error already flattened to words (by a worker, or a crate that only gives text):
/// cut short, without " (os error N)", and never a Debug dump.
pub(crate) fn text_reason(s: &str) -> String {
    let s = s.trim();
    if s.contains(specular_core::net::off::MESSAGE) {
        return t!("error-reason-network-off");
    }
    // A Debug dump ("Os { code: 2, kind: NotFound, … }") reads as nothing to a user.
    if s.contains(" { ") || s.contains("kind: ") || s.contains("Some(") {
        return t!("error-reason-internal");
    }
    system_words(&detail(s))
}

/// A decoder's own detail ("bad Huffman table"): kept as it is (it is what a bug report
/// needs) but cut short, since it is technical and in English.
fn detail(s: &str) -> String {
    const MAX: usize = 80;
    let s = s.trim();
    if s.chars().count() <= MAX {
        return s.to_owned();
    }
    let mut out: String = s.chars().take(MAX - 1).collect();
    out.push('…');
    out
}

/// The system's text of an error without " (os error N)", starting lower case, since it
/// follows a colon.
fn system_words(s: &str) -> String {
    let s = match s.rfind(" (os error ") {
        Some(i) if s.ends_with(')') => &s[..i],
        _ => s,
    };
    let mut c = s.chars();
    match c.next() {
        // "No such device" → "no such device"; "WIC" and other acronyms stay.
        Some(f) if f.is_uppercase() && c.clone().next().is_some_and(char::is_lowercase) => {
            f.to_lowercase().chain(c).collect()
        }
        _ => s.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Error;

    #[test]
    fn io_errors_read_as_reasons() {
        let nf = io_reason(&Error::from(ErrorKind::NotFound));
        assert_eq!(nf, t!("error-reason-not-found"));
        assert_eq!(
            io_reason(&Error::from(ErrorKind::PermissionDenied)),
            t!("error-reason-permission")
        );
        // Unknown kinds keep the system's words without the number.
        let other = io_reason(&Error::other("Device or resource busy (os error 16)"));
        assert_eq!(other, "device or resource busy");
        assert_eq!(system_words("WIC failed"), "WIC failed");
        for r in [
            nf,
            io_reason(&Error::from(ErrorKind::StorageFull)),
            io_reason(&Error::other("Broken pipe (os error 32)")),
        ] {
            assert!(!r.contains("os error") && !r.contains("Os {"), "{r}");
        }
    }

    #[test]
    fn needs_memory_reads_in_free_ram() {
        let gb = 1_000_000_000;
        let n = specular_core::NeedsMemory {
            needed: 24 * gb,
            free: 18 * gb,
        };
        let plain = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
        let r = plain(decode_reason(&DecodeError::NeedsMemory(n)));
        assert!(r.contains("24 GB") && r.contains("18 GB"), "{r}");
        assert!(r.contains("free RAM"), "{r}");
        let full = plain(needs_memory_text(n));
        assert!(
            full.starts_with("This image needs 24 GB of free RAM"),
            "{full}"
        );
        assert_eq!(plain(needs_memory_short(n)), "Needs 24 GB of free RAM");
        // A research volume over the budget says the same (not "damaged" or "unsupported").
        let v = plain(medvol_reason(&specular_decode::medvol::Error::NeedsMemory(
            n,
        )));
        assert_eq!(v, r);
        // Close amounts never read the same: needed rounds up, free down.
        let close = specular_core::NeedsMemory {
            needed: 18_200_000_000,
            free: 17_900_000_000,
        };
        let (a, b) = memory_amounts(close);
        assert_ne!(a, b);
        assert_eq!((plain(a), plain(b)), ("19 GB".into(), "17 GB".into()));
        let small = specular_core::NeedsMemory {
            needed: 2_510_000_000,
            free: 2_490_000_000,
        };
        let (a, b) = memory_amounts(small);
        assert_eq!((plain(a), plain(b)), ("2.6 GB".into(), "2.4 GB".into()));
    }

    #[test]
    fn decode_errors_read_as_reasons() {
        let long = "x".repeat(300);
        let r = decode_reason(&DecodeError::Malformed(long));
        assert!(r.chars().count() < 160, "{r}");
        assert!(r.ends_with('…') || r.contains('…'), "{r}");
        assert_eq!(
            decode_reason(&DecodeError::Io(Error::from(ErrorKind::UnexpectedEof))),
            t!("error-reason-truncated")
        );
        assert_eq!(
            decode_reason(&DecodeError::UnsupportedFormat),
            t!("error-reason-unsupported")
        );
        for e in [
            DecodeError::TooLarge,
            DecodeError::Truncated("eof in scan".into()),
            DecodeError::Unsupported("12-bit lossless".into()),
        ] {
            let r = decode_reason(&e);
            assert!(
                !r.starts_with("truncated:") && !r.starts_with("unsupported:"),
                "{r}"
            );
        }
    }

    /// No reason is a Debug dump, an "os error" number or a variant's name.
    fn plain(r: &str) {
        for bad in ["os error", "Os {", "kind:", "Some(", "Error::", "{ "] {
            assert!(!r.contains(bad), "{r:?} has {bad:?}");
        }
        assert!(!r.is_empty());
    }

    #[test]
    fn dicom_errors_read_as_reasons() {
        use specular_dicom::Error as D;
        assert_eq!(dicom_reason(&D::NotDicom), t!("error-reason-not-dicom"));
        assert_eq!(dicom_reason(&D::Truncated), t!("error-reason-truncated"));
        assert_eq!(dicom_reason(&D::NoPixelData), t!("error-reason-no-image"));
        assert_eq!(dicom_reason(&D::Cancelled), t!("error-reason-cancelled"));
        let r = dicom_reason(&D::Codec("JPEG 2000 part 2".into()));
        assert!(r.contains("JPEG 2000 part 2"), "{r}");
        let r = dicom_reason(&D::Malformed(Box::leak("y".repeat(200).into_boxed_str())));
        assert!(r.chars().count() < 160, "{r}");
        plain(&r);
    }

    #[test]
    fn archive_errors_read_as_reasons() {
        use specular_archive::Error as A;
        assert_eq!(
            archive_reason(&A::Io(Error::from(ErrorKind::NotFound))),
            t!("error-reason-not-found")
        );
        assert_eq!(
            archive_reason(&A::WrongPassword),
            t!("error-reason-wrong-password")
        );
        assert_eq!(archive_reason(&A::Encrypted), t!("error-reason-encrypted"));
        assert_eq!(
            archive_reason(&A::NoEntry(7)),
            t!("error-reason-missing-entry")
        );
        assert_eq!(
            archive_reason(&A::Limit("entry of 9 GB".into())),
            t!("error-reason-too-large")
        );
        for e in [
            A::NotAnArchive,
            A::Checksum,
            A::Malformed("bad header".into()),
        ] {
            plain(&archive_reason(&e));
        }
    }

    #[test]
    fn volume_refusals_read_as_reasons() {
        use specular_dicom::volume::Refusal as R;
        let r = refusal_reason(&R::TooFew(2));
        assert!(r.contains('2'), "{r}");
        let r = refusal_reason(&R::TooLarge {
            need: 3 << 30,
            budget: 1 << 30,
        });
        // In MB, not bytes.
        let digits: String = r
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == ' ')
            .collect();
        assert!(digits.contains("3072") && digits.contains("1024"), "{r}");
        for e in [
            R::NoGeometry,
            R::NotParallel,
            R::Mismatch,
            R::NotGrey,
            R::Missing(4),
        ] {
            plain(&refusal_reason(&e));
        }
    }

    #[test]
    fn url_errors_read_as_reasons() {
        use specular_platform::url_scheme::Error as U;
        assert_eq!(
            url_reason(&U::BadCharacter(17)),
            t!("error-reason-url-damaged")
        );
        assert_eq!(
            url_reason(&U::UnknownAction("frobnicate".into())),
            t!("error-reason-url-unknown")
        );
        // Byte offsets and parameter names stay in the log.
        for e in [
            U::BadEscape(3),
            U::UnknownParameter("zz".into()),
            U::TooManyPaths,
        ] {
            let r = url_reason(&e);
            assert!(!r.contains('3') && !r.contains("zz"), "{r}");
            plain(&r);
        }
    }

    #[test]
    fn library_errors_read_as_reasons() {
        use rusqlite::ffi;
        use specular_library::Error as L;
        let sql = |code| L::Sql(rusqlite::Error::SqliteFailure(ffi::Error::new(code), None));
        assert_eq!(
            library_reason(&sql(ffi::SQLITE_BUSY)),
            t!("error-reason-busy")
        );
        assert_eq!(
            library_reason(&sql(ffi::SQLITE_FULL)),
            t!("error-reason-disk-full")
        );
        assert_eq!(
            library_reason(&sql(ffi::SQLITE_CORRUPT)),
            t!("error-reason-catalogue-damaged")
        );
        assert_eq!(
            library_reason(&sql(ffi::SQLITE_READONLY)),
            t!("error-reason-read-only")
        );
        assert_eq!(
            library_reason(&L::Newer { found: 9 }),
            t!("error-reason-newer-version")
        );
        assert_eq!(
            library_reason(&L::Io(Error::from(ErrorKind::PermissionDenied))),
            t!("error-reason-permission")
        );
        // SQLite's own words ("no such column: x") are a bug, not something to fix.
        assert_eq!(
            library_reason(&L::Sql(rusqlite::Error::InvalidColumnName("x".into()))),
            t!("error-reason-internal")
        );
    }

    #[test]
    fn medical_volume_errors_read_as_reasons() {
        use specular_decode::medvol::Error as M;
        assert_eq!(
            medvol_reason(&M::Truncated("header".into())),
            t!("error-reason-truncated")
        );
        assert_eq!(medvol_reason(&M::TooLarge), t!("error-reason-too-large"));
        assert_eq!(
            medvol_reason(&M::NeedsLayout),
            t!("error-reason-needs-layout")
        );
        assert_eq!(
            medvol_reason(&M::Io("No such file or directory (os error 2)".into())),
            "no such file or directory"
        );
    }

    /// Any error: the first in its chain with a reason of its own; else its words, cleaned.
    #[test]
    fn any_error_reads_through_its_chain() {
        #[derive(Debug)]
        struct Outer(Error);
        impl std::fmt::Display for Outer {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "saving the report: {:?}", self.0)
            }
        }
        impl std::error::Error for Outer {
            fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
                Some(&self.0)
            }
        }
        let e = Outer(Error::from(ErrorKind::StorageFull));
        assert_eq!(reason(&e), io_reason(&Error::from(ErrorKind::StorageFull)));
        let lib = specular_library::Error::Newer { found: 4 };
        assert_eq!(reason(&lib), t!("error-reason-newer-version"));
        // Nothing known in the chain: its own words.
        let e = std::fmt::Error;
        plain(&reason(&e));
    }

    #[test]
    fn flattened_text_is_never_a_debug_dump() {
        assert_eq!(
            text_reason("Os { code: 2, kind: NotFound, message: \"No such file\" }"),
            t!("error-reason-internal")
        );
        assert_eq!(text_reason("Some(Truncated)"), t!("error-reason-internal"));
        assert_eq!(
            text_reason("  Permission denied (os error 13) "),
            "permission denied"
        );
        let r = text_reason(&"z".repeat(500));
        assert!(r.chars().count() <= 80, "{}", r.chars().count());
    }
}
