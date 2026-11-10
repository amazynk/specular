## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } riga
   *[other] { $count } righe
}
text-view-rows = { $rows ->
    [one] { $rows } riga
   *[other] { $rows } righe
} × { $columns ->
    [one] { $columns } colonna
   *[other] { $columns } colonne
}
text-view-cues = { $count ->
    [one] { $count } sottotitolo
   *[other] { $count } sottotitoli
}
text-view-wrap = A capo automatico
text-view-show-source = Mostra sorgente
text-view-show-rendered = Mostra formattato
text-view-text = Testo di { $name }
text-view-markdown = Documento Markdown { $name }
text-view-table = Tabella { $name }
text-view-subtitles = Sottotitoli di { $name }
text-view-picture-cue = Sottotitolo in immagine
