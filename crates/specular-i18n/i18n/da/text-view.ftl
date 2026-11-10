## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } linje
   *[other] { $count } linjer
}
text-view-rows = { $rows ->
    [one] { $rows } række
   *[other] { $rows } rækker
} × { $columns ->
    [one] { $columns } kolonne
   *[other] { $columns } kolonner
}
text-view-cues = { $count ->
    [one] { $count } undertekst
   *[other] { $count } undertekster
}
text-view-wrap = Ombryd linjer
text-view-show-source = Vis kilde
text-view-show-rendered = Vis formateret
text-view-text = Tekst i { $name }
text-view-markdown = Markdown-dokument { $name }
text-view-table = Tabel { $name }
text-view-subtitles = Undertekster i { $name }
text-view-picture-cue = Billedundertekst
