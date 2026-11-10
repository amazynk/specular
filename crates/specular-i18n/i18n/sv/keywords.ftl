# The keyword panel (gallery, Library, cull mode).

palette-keywords = Nyckelord
palette-keywords-help = Ge de markerade bilderna nyckelord.
keywords-title = Nyckelord
keywords-close = Stäng nyckelord
keywords-photos = { $count ->
    [one] 1 bild
   *[other] { $count } bilder
}
keywords-field = Lägg till ett nyckelord
keywords-synonyms-of = Synonymer till { $keyword }, åtskilda med kommatecken
keywords-none-selected = Markera bilder för att ge dem nyckelord.
keywords-reading = Läser nyckelord…
keywords-on-photos = På de här bilderna
keywords-on-none = Inga nyckelord än.
keywords-suggestions = Förslag
keywords-recent = Senast använda
keywords-list = Nyckelordslista
keywords-list-empty = Nyckelord du lägger till visas här. Du kan också importera en lista från Lightroom.
keywords-create = Lägg till ”{ $keyword }”
keywords-count = { $count } av { $total }
keywords-private = Privat
keywords-private-mark = privat
keywords-synonyms = Synonymer
keywords-forget = Ta bort från listan
keywords-import = Importera lista…
keywords-export = Exportera lista…
keywords-forget-ask = Ta bort ”{ $keyword }” och nyckelorden under det från listan?
keywords-forget-keep = Bilderna behåller sina nyckelord.
keywords-forget-yes = Ta bort
keywords-forgotten = { $count ->
    [one] Tog bort { $keyword } från listan.
   *[other] Tog bort { $keyword } och nyckelorden under det ({ $count } totalt) från listan.
}
keywords-added = Lade till { $keyword }.
keywords-removed = Tog bort { $keyword }.
keywords-imported = { $count ->
    [one] 1 nytt nyckelord från { $file }.
   *[other] { $count } nya nyckelord från { $file }.
}
keywords-import-failed = Kunde inte läsa { $file }: { $error }
keywords-exported = Sparade nyckelordslistan som { $file }.
keywords-export-failed = Kunde inte spara nyckelordslistan: { $error }
keywords-write-failed = { $count ->
    [one] Nyckelorden sparades inte för 1 bild: { $error }
   *[other] Nyckelorden sparades inte för { $count } bilder: { $error }
}
keywords-dialog-import = Importera en nyckelordslista från Lightroom
keywords-dialog-export = Exportera nyckelordslistan
keywords-file-kind = Nyckelordslista
keywords-file-name = Nyckelord
