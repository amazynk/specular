## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } řádek
    [few] { $count } řádky
    [many] { $count } řádku
   *[other] { $count } řádků
}
text-view-rows = { $rows ->
    [one] { $rows } řádek
    [few] { $rows } řádky
    [many] { $rows } řádku
   *[other] { $rows } řádků
} × { $columns ->
    [one] { $columns } sloupec
    [few] { $columns } sloupce
    [many] { $columns } sloupce
   *[other] { $columns } sloupců
}
text-view-cues = { $count ->
    [one] { $count } titulek
    [few] { $count } titulky
    [many] { $count } titulku
   *[other] { $count } titulků
}
text-view-wrap = Zalamovat řádky
text-view-show-source = Zobrazit zdroj
text-view-show-rendered = Zobrazit formátované
text-view-text = Text souboru { $name }
text-view-markdown = Dokument Markdown { $name }
text-view-table = Tabulka { $name }
text-view-subtitles = Titulky souboru { $name }
text-view-picture-cue = Obrázkový titulek
