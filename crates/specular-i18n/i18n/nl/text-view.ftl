## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } regel
   *[other] { $count } regels
}
text-view-rows = { $rows ->
    [one] { $rows } rij
   *[other] { $rows } rijen
} × { $columns ->
    [one] { $columns } kolom
   *[other] { $columns } kolommen
}
text-view-cues = { $count ->
    [one] { $count } ondertitel
   *[other] { $count } ondertitels
}
text-view-wrap = Regels laten teruglopen
text-view-show-source = Bron tonen
text-view-show-rendered = Opgemaakt tonen
text-view-text = Tekst van { $name }
text-view-markdown = Markdown-document { $name }
text-view-table = Tabel { $name }
text-view-subtitles = Ondertitels van { $name }
text-view-picture-cue = Ondertitel als afbeelding
