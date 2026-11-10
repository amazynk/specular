## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } rad
   *[other] { $count } rader
}
text-view-rows = { $rows ->
    [one] { $rows } rad
   *[other] { $rows } rader
} × { $columns ->
    [one] { $columns } kolumn
   *[other] { $columns } kolumner
}
text-view-cues = { $count ->
    [one] { $count } undertext
   *[other] { $count } undertexter
}
text-view-wrap = Radbryt
text-view-show-source = Visa källa
text-view-show-rendered = Visa formaterad
text-view-text = Text i { $name }
text-view-markdown = Markdown-dokument { $name }
text-view-table = Tabell { $name }
text-view-subtitles = Undertexter i { $name }
text-view-picture-cue = Bildundertext
