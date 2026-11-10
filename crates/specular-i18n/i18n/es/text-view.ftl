## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } línea
   *[other] { $count } líneas
}
text-view-rows = { $rows ->
    [one] { $rows } fila
   *[other] { $rows } filas
} × { $columns ->
    [one] { $columns } columna
   *[other] { $columns } columnas
}
text-view-cues = { $count ->
    [one] { $count } subtítulo
   *[other] { $count } subtítulos
}
text-view-wrap = Ajustar líneas
text-view-show-source = Mostrar código fuente
text-view-show-rendered = Mostrar con formato
text-view-text = Texto de { $name }
text-view-markdown = Documento Markdown { $name }
text-view-table = Tabla { $name }
text-view-subtitles = Subtítulos de { $name }
text-view-picture-cue = Subtítulo de imagen
