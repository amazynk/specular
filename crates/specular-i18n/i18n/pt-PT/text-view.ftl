## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } linha
   *[other] { $count } linhas
}
text-view-rows = { $rows ->
    [one] { $rows } linha
   *[other] { $rows } linhas
} × { $columns ->
    [one] { $columns } coluna
   *[other] { $columns } colunas
}
text-view-cues = { $count ->
    [one] { $count } legenda
   *[other] { $count } legendas
}
text-view-wrap = Quebrar linhas
text-view-show-source = Mostrar código-fonte
text-view-show-rendered = Mostrar formatado
text-view-text = Texto de { $name }
text-view-markdown = Documento Markdown { $name }
text-view-table = Tabela { $name }
text-view-subtitles = Legendas de { $name }
text-view-picture-cue = Legenda em imagem
