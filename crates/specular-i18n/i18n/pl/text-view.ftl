## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } wiersz
    [few] { $count } wiersze
    [many] { $count } wierszy
   *[other] { $count } wiersza
}
text-view-rows = { $rows ->
    [one] { $rows } wiersz
    [few] { $rows } wiersze
    [many] { $rows } wierszy
   *[other] { $rows } wiersza
} × { $columns ->
    [one] { $columns } kolumna
    [few] { $columns } kolumny
    [many] { $columns } kolumn
   *[other] { $columns } kolumny
}
text-view-cues = { $count ->
    [one] { $count } napis
    [few] { $count } napisy
    [many] { $count } napisów
   *[other] { $count } napisu
}
text-view-wrap = Zawijaj wiersze
text-view-show-source = Pokaż źródło
text-view-show-rendered = Pokaż sformatowany
text-view-text = Tekst pliku { $name }
text-view-markdown = Dokument Markdown { $name }
text-view-table = Tabela { $name }
text-view-subtitles = Napisy pliku { $name }
text-view-picture-cue = Napis graficzny
