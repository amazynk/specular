//! The [`Command::Item`](specular_widgets::Command::Item) ids the app's confirmations send,
//! in one list: two features once shared an id, and accepting "Add your Pictures folder" on
//! the start screen went to the Library's removal instead. A new confirmation takes the
//! next free id here, and `ids_are_unique` checks it against the rest.

const BASE: u64 = 1 << 45;

/// "Open this link?" (remote URLs).
pub(in crate::app) const REMOTE_CONFIRM: u64 = BASE + 1;
/// "Remove … from the Library?"
pub(in crate::app) const LIBRARY_REMOVE: u64 = BASE + 2;
/// "Add your Pictures folder to the library?" on the start screen.
pub(in crate::app) const WELCOME_PICTURES: u64 = BASE + 3;
/// "Add all of your home folder?": Add Pictures instead.
pub(in crate::app) const LIBRARY_HOME_PICTURES: u64 = BASE + 4;
/// "Add all of your home folder?": add it anyway.
pub(in crate::app) const LIBRARY_HOME_KEEP: u64 = BASE + 5;
/// "Remove this map service?"
pub(in crate::app) const MAP_SERVICE_REMOVE: u64 = BASE + 0x51;
/// "Write positions to these photos?"
pub(in crate::app) const MAP_GEOTAG: u64 = BASE + 0x52;
/// "Forget this optional decoder?"
pub(in crate::app) const DECODER_FORGET: u64 = BASE + 0xD0;
/// Cull mode on subframes: "Reject the suggested frames?"
pub(in crate::app) const CULL_AUTO_REJECT: u64 = BASE + 0xE0;
/// Cull mode on subframes: "Move the rejected frames to …?"
pub(in crate::app) const CULL_MOVE_REJECTS: u64 = BASE + 0xE1;
/// "Run this recipe?" (its first run).
pub(in crate::app) const RECIPE_CONFIRM: u64 = BASE + 0xE2;
/// "Delete this album?" (Library albums; the photos stay).
pub(in crate::app) const ALBUM_DELETE: u64 = BASE + 0x120;
/// "Remove these photos from the album?" (the files stay).
pub(in crate::app) const ALBUM_REMOVE_PHOTOS: u64 = BASE + 0x121;
/// "Delete this markup?"
pub(in crate::app) const MARKUP_DELETE: u64 = BASE + 0xE3;
/// "Show downloads" in the palette.
pub(in crate::app) const DOWNLOADS_SHOW: u64 = BASE + 0xE4;
/// "Remove this view?" (a model's saved views).
pub(in crate::app) const BIM_VIEW_DELETE: u64 = BASE + 0xE5;
/// "Use an unverified model?" (Local AI, Install from file).
pub(in crate::app) const AI_UNVERIFIED: u64 = BASE + 0xE6;
/// "Delete the centreline …?" (curved MPR).
pub(in crate::app) const CPR_DELETE: u64 = BASE + 0xE7;
/// "Open this link from a code?" (Read codes).
pub(in crate::app) const CODES_OPEN: u64 = BASE + 0xE8;
/// "Send with Zynk needs the Zynk app": Get Zynk.
pub(in crate::app) const ZYNK_GET: u64 = BASE + 0xE9;
/// "Remove this model from the scene?" (several IFC models in one scene).
pub(in crate::app) const BIM_MODEL_REMOVE: u64 = BASE + 0xEA;
/// "Place it at the origin?" (an IFC model that cannot be aligned).
pub(in crate::app) const BIM_MODEL_ORIGIN: u64 = BASE + 0xEB;
/// "Remove this add-on?" (Local AI, a script's text recogniser).
pub(in crate::app) const AI_ADDON_REMOVE: u64 = BASE + 0xEC;
/// "Forget where videos were left?" (Settings › Viewing).
pub(in crate::app) const PLAYBACK_FORGET: u64 = BASE + 0xF0;
/// "Clear the play queue?"
pub(in crate::app) const QUEUE_CLEAR: u64 = BASE + 0xF1;
/// "Delete the preset?" (the equaliser's panel).
pub(in crate::app) const EQ_PRESET_DELETE: u64 = BASE + 0xF2;
/// "Forget this stream?" (the recent streams of Open network stream).
pub(in crate::app) const STREAM_FORGET: u64 = BASE + 0xF3;
/// Develop: a deletion asked about (a style, preset, snapshot, version or rule).
pub(in crate::app) const DEVELOP_CONFIRM: u64 = BASE + 0xF4;
/// Develop: the sidecar changed outside Specular; use the file's edit, or keep the library's.
pub(in crate::app) const DEVELOP_CONFLICT_FILE: u64 = BASE + 0xF8;
pub(in crate::app) const DEVELOP_CONFLICT_LIBRARY: u64 = BASE + 0xF9;
/// A batch's answer about photos whose sidecar changed outside Specular.
pub(in crate::app) const DEVELOP_BATCH_FILE: u64 = BASE + 0xFA;
pub(in crate::app) const DEVELOP_BATCH_LIBRARY: u64 = BASE + 0xFB;
pub(in crate::app) const DEVELOP_BATCH_SKIP: u64 = BASE + 0xFC;
/// Develop's menu rows: this and the 0x1_0000 ids after it (`develop/batch.rs`).
pub(in crate::app) const DEVELOP: u64 = BASE + 0x1_0000;
/// "Remove this mask?", "Delete this instance?" (Develop's Masks section).
pub(in crate::app) const DEVELOP_MASK_REMOVE: u64 = BASE + 0x130;
/// Develop's picture menu (clipping, soft proof, scopes, print): this and the 15 after it.
pub(in crate::app) const DEVELOP_AIDS: u64 = BASE + 0x140;
/// People: "Merge … into …?" after dragging one person onto another.
pub(in crate::app) const PEOPLE_MERGE: u64 = BASE + 0x150;
/// "Forget this file kind rule?" (Settings › Optional decoders › File kinds).
pub(in crate::app) const FILE_KIND_FORGET: u64 = BASE + 0x168;
/// "Keep a local copy of …?": the originals, or proxies of the raws.
pub(in crate::app) const LOCAL_KEEP_ORIGINALS: u64 = BASE + 0x170;
pub(in crate::app) const LOCAL_KEEP_PROXIES: u64 = BASE + 0x171;
/// "Remove the local copy of …?" (the originals stay).
pub(in crate::app) const LOCAL_REMOVE: u64 = BASE + 0x172;
/// A file changed while offline: keep the change made here, or the file's.
pub(in crate::app) const LOCAL_MINE: u64 = BASE + 0x173;
pub(in crate::app) const LOCAL_THEIRS: u64 = BASE + 0x174;
/// "Export from proxies?": now from the proxies, or wait for the originals.
pub(in crate::app) const LOCAL_EXPORT_PROXY: u64 = BASE + 0x175;
pub(in crate::app) const LOCAL_EXPORT_WAIT: u64 = BASE + 0x176;
/// "Run this hook …?" (its first run): Allow, or Not now.
pub(in crate::app) const HOOK_ALLOW: u64 = BASE + 0x1A0;
pub(in crate::app) const HOOK_NOT_NOW: u64 = BASE + 0x1A1;
/// "Remove the hook …?" (Settings › Integration; the program and files stay).
pub(in crate::app) const HOOK_REMOVE: u64 = BASE + 0x1A2;
/// "Turn off desktop search?" (Settings › Integration; the Library and pictures stay).
pub(in crate::app) const INTEGRATION_SEARCH_REMOVE: u64 = BASE + 0x1A3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let ids = [
            REMOTE_CONFIRM,
            LIBRARY_REMOVE,
            WELCOME_PICTURES,
            LIBRARY_HOME_PICTURES,
            LIBRARY_HOME_KEEP,
            MAP_SERVICE_REMOVE,
            MAP_GEOTAG,
            DECODER_FORGET,
            CULL_AUTO_REJECT,
            PEOPLE_MERGE,
            FILE_KIND_FORGET,
            LOCAL_KEEP_ORIGINALS,
            LOCAL_KEEP_PROXIES,
            LOCAL_REMOVE,
            LOCAL_MINE,
            LOCAL_THEIRS,
            LOCAL_EXPORT_PROXY,
            LOCAL_EXPORT_WAIT,
            CULL_MOVE_REJECTS,
            RECIPE_CONFIRM,
            DEVELOP_MASK_REMOVE,
            MARKUP_DELETE,
            DOWNLOADS_SHOW,
            BIM_VIEW_DELETE,
            AI_UNVERIFIED,
            CPR_DELETE,
            CODES_OPEN,
            ZYNK_GET,
            BIM_MODEL_REMOVE,
            BIM_MODEL_ORIGIN,
            AI_ADDON_REMOVE,
            PLAYBACK_FORGET,
            QUEUE_CLEAR,
            EQ_PRESET_DELETE,
            STREAM_FORGET,
            DEVELOP_CONFIRM,
            DEVELOP_CONFLICT_FILE,
            DEVELOP_CONFLICT_LIBRARY,
            DEVELOP_BATCH_FILE,
            DEVELOP_BATCH_LIBRARY,
            DEVELOP_BATCH_SKIP,
            DEVELOP_AIDS,
            crate::app::insights::ITEM,
            crate::app::work::ITEM,
            ALBUM_DELETE,
            ALBUM_REMOVE_PHOTOS,
            HOOK_ALLOW,
            HOOK_NOT_NOW,
            HOOK_REMOVE,
            INTEGRATION_SEARCH_REMOVE,
        ];
        for (i, a) in ids.iter().enumerate() {
            assert!(!ids[i + 1..].contains(a), "item id {a:#x} is used twice");
            // Nor in the gallery's range of codes.
            let g = crate::app::gallery::ITEM_BASE;
            assert!(
                !(g..g + 1000).contains(a),
                "{a:#x} is in the gallery's range"
            );
        }
    }
}
