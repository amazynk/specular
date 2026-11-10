## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } Zeile
   *[other] { $count } Zeilen
}
text-view-rows = { $rows ->
    [one] { $rows } Zeile
   *[other] { $rows } Zeilen
} × { $columns ->
    [one] { $columns } Spalte
   *[other] { $columns } Spalten
}
text-view-cues = { $count ->
    [one] { $count } Einblendung
   *[other] { $count } Einblendungen
}
text-view-wrap = Zeilen umbrechen
text-view-show-source = Quelltext zeigen
text-view-show-rendered = Formatiert zeigen
text-view-text = Text von { $name }
text-view-markdown = Markdown-Dokument { $name }
text-view-table = Tabelle { $name }
text-view-subtitles = Untertitel von { $name }
text-view-picture-cue = Bilduntertitel
