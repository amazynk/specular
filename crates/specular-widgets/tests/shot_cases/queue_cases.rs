use specular_settings::PlayRepeat;
use specular_widgets::{PlayOrder, QueueRow, QueueView};

/// A queue as the app words it (`app/queue.rs`), in the case's language: an album playing
/// its second track, items without a length yet, a long title, a video, and a network
/// item from a playlist.
pub(crate) fn queue_view() -> QueueView {
    use specular_i18n::t;
    let row = |title: &str, detail: &str, duration: &str, current: bool| QueueRow {
        title: title.into(),
        detail: detail.into(),
        duration: duration.into(),
        current,
        playable: true,
    };
    let rows = vec![
        row("Nina Simone – Feeling Good", "Best of", "2:53", false),
        row("Nina Simone – Sinnerman", "Best of", "10:21", true),
        row("03 Wild Is the Wind.flac", "Best of", "", false),
        row(
            "Ofra Haza – Im Nin’alu (Played in Full Mix, Extended Remastered Version 2024)",
            "שירים",
            "8:04",
            false,
        ),
        row("concert-2019-08-12.mp4", "Concerts 2019", "1:12:40", false),
        QueueRow {
            title: "http://radio.example.org:8000/live".into(),
            detail: t!(
                "queue-network-item",
                address = "http://radio.example.org:8000/live"
            ),
            duration: String::new(),
            current: false,
            playable: false,
        },
    ];
    QueueView {
        summary: t!("queue-summary", count = rows.len() as i64),
        rows,
    }
}

/// Repeat all and shuffle on, as the transport and the panel show them.
pub(crate) fn play_order() -> PlayOrder {
    PlayOrder {
        repeat: PlayRepeat::All,
        shuffle: true,
    }
}

/// Repeat one and shuffle off, on a sound file's transport.
pub(crate) fn play_order_one() -> PlayOrder {
    PlayOrder {
        repeat: PlayRepeat::One,
        shuffle: false,
    }
}
