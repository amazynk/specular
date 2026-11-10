# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

cheatsheet-video-queue = Queue, repeat and shuffle

## Command palette

palette-next-track = Next item
palette-next-track-help = Play the next item of the queue, or the folder’s next file of the same kind.
palette-prev-track = Previous item
palette-prev-track-help = Play the previous item. After 3 seconds, start this one over.
palette-cycle-repeat = Repeat
palette-cycle-repeat-help = Repeat off, all or one.
palette-shuffle = Shuffle
palette-shuffle-help = Play in a random order, each item once a round.
palette-play-queue = Play queue
palette-play-queue-help = Show what plays next: reorder, remove, save.
palette-queue-play-next = Play next
palette-queue-play-next-help = Play the file (in the gallery, the selection) after the item playing.
palette-queue-add = Add to queue
palette-queue-add-help = Add the file (in the gallery, the selection) at the end of the queue.
palette-save-queue = Save queue as playlist…
palette-save-queue-help = Save the queue as an M3U8 or XSPF playlist.

## The sound transport

transport-prev-track = Previous item
transport-next-track = Next item

## The queue panel

queue-title = Play queue
queue-summary = { $count ->
    [one] 1 item
   *[other] { $count } items
}
queue-summary-time = { $count ->
    [one] 1 item
   *[other] { $count } items
} · { $time }
queue-empty = The queue is empty.
    Add sound files and videos with Play next or Add to queue, or open a playlist.
queue-empty-short = The queue is empty.
queue-playing = Playing
queue-repeat = Repeat
queue-repeat-off = Repeat: off
queue-repeat-all = Repeat: all
queue-repeat-one = Repeat: one
queue-shuffle = Shuffle
queue-shuffle-on = Shuffle: on
queue-shuffle-off = Shuffle: off
queue-save = Save as playlist…
queue-save-tip = Save the queue as an M3U8 or XSPF playlist
queue-clear = Clear…
queue-clear-tip = Empty the queue. Files stay on disk.
queue-remove = Remove from queue (file stays on disk)
queue-network-item = Network item, not played: { $address }
queue-artist-title = { $artist } – { $title }

## Toasts and questions

queue-at-end = No next item.
queue-at-start = No previous item.
queue-missing = { $name } is no longer there. Skipped.
queue-network-not-played = Network items do not play yet.
queue-nothing-to-add = Nothing to add. Only sound files, videos and playlists go into the queue.
queue-added = { $count ->
    [one] Added 1 item to the queue.
   *[other] Added { $count } items to the queue.
}
queue-added-next = { $count ->
    [one] 1 item plays next.
   *[other] { $count } items play next.
}
queue-playlist-opened = { $name }: { $count ->
    [one] 1 item in the queue.
   *[other] { $count } items in the queue.
}
queue-playlist-nothing = Nothing in { $name } plays here. The queue lists what it holds.
queue-playlist-failed = Can’t open { $name }: { $reason }
queue-playlist-unreadable = the file can’t be read.
queue-playlist-damaged = it isn’t a playlist Specular can read.
queue-clear-ask = { $count ->
    [one] Clear the queue (1 item)?
   *[other] Clear the queue ({ $count } items)?
}
queue-clear-detail = Only the list is cleared. The files stay on disk.
queue-clear-ok = Clear queue
queue-cleared = Queue cleared.
queue-save-title = Save queue as playlist
queue-default-name = Queue
queue-format-m3u8 = M3U8 playlist
queue-format-xspf = XSPF playlist
queue-saved = Saved { $name }.
queue-save-failed = Can’t save { $name }: { $reason }

## Settings › Viewing

settings-viewing-sound-play-next = Play the next sound file
settings-viewing-sound-play-next-help = When a sound file ends, the next sound file in the folder plays.
settings-viewing-play-repeat = Repeat
settings-viewing-play-repeat-help = For sound files, videos and the queue.
settings-viewing-play-repeat-off = Off
settings-viewing-play-repeat-all = All
settings-viewing-play-repeat-one = One
settings-viewing-play-shuffle = Shuffle
settings-viewing-play-shuffle-help = Play in a random order, each item once a round.
