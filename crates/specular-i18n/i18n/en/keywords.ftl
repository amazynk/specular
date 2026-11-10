# The keyword panel (gallery, Library, cull mode).

palette-keywords = Keywords
palette-keywords-help = Tag the selected photos with keywords.
keywords-title = Keywords
keywords-close = Close keywords
keywords-photos = { $count ->
    [one] 1 photo
   *[other] { $count } photos
}
keywords-field = Add a keyword
keywords-synonyms-of = Synonyms of { $keyword }, separated by commas
keywords-none-selected = Select photos to tag them.
keywords-reading = Reading keywords…
keywords-on-photos = On these photos
keywords-on-none = No keywords yet.
keywords-suggestions = Suggestions
keywords-recent = Recently used
keywords-list = Keyword list
keywords-list-empty = Keywords you add appear here. You can also import a Lightroom list.
keywords-create = Add “{ $keyword }”
keywords-count = { $count } of { $total }
keywords-private = Private
keywords-private-mark = private
keywords-synonyms = Synonyms
keywords-forget = Remove from list
keywords-import = Import list…
keywords-export = Export list…
keywords-forget-ask = Remove “{ $keyword }” and the keywords below it from the list?
keywords-forget-keep = Photos keep their keywords.
keywords-forget-yes = Remove
keywords-forgotten = { $count ->
    [one] Removed { $keyword } from the list.
   *[other] Removed { $keyword } and the keywords under it ({ $count } in all) from the list.
}
keywords-added = Added { $keyword }.
keywords-removed = Removed { $keyword }.
keywords-imported = { $count ->
    [one] 1 new keyword from { $file }.
   *[other] { $count } new keywords from { $file }.
}
keywords-import-failed = Could not read { $file }: { $error }
keywords-exported = Saved the keyword list as { $file }.
keywords-export-failed = Could not save the keyword list: { $error }
keywords-write-failed = { $count ->
    [one] Keywords not saved for 1 photo: { $error }
   *[other] Keywords not saved for { $count } photos: { $error }
}
keywords-dialog-import = Import a Lightroom keyword list
keywords-dialog-export = Export the keyword list
keywords-file-kind = Keyword list
keywords-file-name = Keywords
