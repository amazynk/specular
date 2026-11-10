# Client galleries: a static HTML folder of photos, and reading the client's picks back.

palette-client-gallery = Client gallery…
palette-client-gallery-help = Write the photos as a web page folder for a client, with picks they can send back.
palette-import-client-picks = Import client picks…
palette-import-client-picks-help = Mark the photos a client picked in their gallery.

webgallery-heading = Client gallery
webgallery-subtitle = { $count ->
    [one] One photo
   *[other] { $count } photos
}
webgallery-title-label = Title
webgallery-title-default = Proofs
webgallery-watermark-label = Watermark text (empty: none)
webgallery-source = Photos
webgallery-source-current = This photo
webgallery-source-selection = Selection
webgallery-source-album = Album “{ $name }”
webgallery-source-search = Search results
webgallery-source-folder = This folder
webgallery-source-picks = Picks
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Theme
webgallery-theme-light = Light
webgallery-theme-dark = Dark
webgallery-theme-minimal = Minimal
webgallery-size = Long edge
webgallery-size-value = { $px } px
webgallery-format = Format
webgallery-captions = Under each photo
webgallery-captions-off = Nothing
webgallery-captions-name = File name
webgallery-captions-title = Title
webgallery-captions-all = File name, title and caption
webgallery-exif = Show camera and exposure
webgallery-location = Keep location (GPS) in the photos
webgallery-note-private = A static gallery cannot be protected by a password.
webgallery-note-share = Anyone with the folder or its address sees every photo. Share the link only with your client, or use your web host's access control.
webgallery-note-picks = Clients pick with a heart. Picks stay in their browser until they send you the list.
webgallery-write = Write gallery…
webgallery-busy = Writing { $done } of { $total }…
webgallery-stop = Stop
webgallery-done = { $count ->
    [one] Gallery written: one photo in { $folder }
   *[other] Gallery written: { $count } photos in { $folder }
}
webgallery-done-failed = { $failed ->
    [one] One photo could not be written.
   *[other] { $failed } photos could not be written.
}
webgallery-stopped = Gallery stopped. Nothing was kept.
webgallery-failed = Gallery not written: { $reason }
webgallery-nothing = No photo could be written.

# The words on the gallery page itself (in the language Specular shows).
webgallery-page-count = { $count ->
    [one] One photo
   *[other] { $count } photos
}
webgallery-page-hint = Tap the heart on the photos you want. Your picks stay in this browser until you send them.
webgallery-page-picked = Picked: {"{"}n{"}"}
webgallery-page-pick = Pick
webgallery-page-copy = Copy my picks
webgallery-page-copied = Copied. Paste the list in a message.
webgallery-page-copy-by-hand = Copy the selected list and paste it in a message.
webgallery-page-download-txt = Download picks.txt
webgallery-page-download-json = Download picks.json
webgallery-page-clear = Clear picks
webgallery-page-clear-ask = Clear all your picks in this browser?
webgallery-page-no-storage = This browser will not keep your picks. Send them before you close the page.
webgallery-page-noscript = Picking needs JavaScript. The photos still open.
webgallery-page-previous = Previous
webgallery-page-next = Next
webgallery-page-close = Close
webgallery-page-position = {"{"}i{"}"} of {"{"}n{"}"}
webgallery-page-viewer = Photo viewer
webgallery-page-picks = Your picks

clientpicks-heading = Import client picks
clientpicks-subtitle = { $count ->
    [one] Matched against one photo shown
   *[other] Matched against the { $count } photos shown
}
clientpicks-paste-label = Names (paste the client's list)
clientpicks-choose = Choose picks file…
clientpicks-from-file = From { $name }
clientpicks-apply-as = Mark them
clientpicks-as-pick = As picks
clientpicks-as-stars = { $stars ->
    [one] One star
   *[other] { $stars } stars
}
clientpicks-as-label = Label { $label }
clientpicks-empty = Paste the list the client sent, or choose their picks.txt or picks.json.
clientpicks-matched = { $matched } of { $total } names match photos here.
clientpicks-missing = Not found: { $names }
clientpicks-ambiguous = In more than one folder, left alone: { $names }
clientpicks-more = { $names } and { $more } more
clientpicks-apply = Apply
clientpicks-applied = { $count ->
    [one] Marked one photo.
   *[other] Marked { $count } photos.
}
clientpicks-unchanged = Those photos were marked already.
clientpicks-read-failed = Can’t read { $name }: { $reason }
