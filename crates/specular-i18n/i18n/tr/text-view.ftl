## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } satır
   *[other] { $count } satır
}
text-view-rows = { $rows ->
    [one] { $rows } satır
   *[other] { $rows } satır
} × { $columns ->
    [one] { $columns } sütun
   *[other] { $columns } sütun
}
text-view-cues = { $count ->
    [one] { $count } altyazı
   *[other] { $count } altyazı
}
text-view-wrap = Satırları kaydır
text-view-show-source = Kaynağı göster
text-view-show-rendered = Biçimli göster
text-view-text = { $name } metni
text-view-markdown = Markdown belgesi { $name }
text-view-table = Tablo { $name }
text-view-subtitles = { $name } altyazıları
text-view-picture-cue = Resimli altyazı
