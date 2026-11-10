## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } rivi
   *[other] { $count } riviä
}
text-view-rows = { $rows ->
    [one] { $rows } rivi
   *[other] { $rows } riviä
} × { $columns ->
    [one] { $columns } sarake
   *[other] { $columns } saraketta
}
text-view-cues = { $count ->
    [one] { $count } tekstitys
   *[other] { $count } tekstitystä
}
text-view-wrap = Rivitä
text-view-show-source = Näytä lähde
text-view-show-rendered = Näytä muotoiltuna
text-view-text = Tiedoston { $name } teksti
text-view-markdown = Markdown-asiakirja { $name }
text-view-table = Taulukko { $name }
text-view-subtitles = Tiedoston { $name } tekstitykset
text-view-picture-cue = Kuvatekstitys
