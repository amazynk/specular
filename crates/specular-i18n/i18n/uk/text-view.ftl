## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } рядок
    [few] { $count } рядки
    [many] { $count } рядків
   *[other] { $count } рядка
}
text-view-rows = { $rows ->
    [one] { $rows } рядок
    [few] { $rows } рядки
    [many] { $rows } рядків
   *[other] { $rows } рядка
} × { $columns ->
    [one] { $columns } стовпець
    [few] { $columns } стовпці
    [many] { $columns } стовпців
   *[other] { $columns } стовпця
}
text-view-cues = { $count ->
    [one] { $count } субтитр
    [few] { $count } субтитри
    [many] { $count } субтитрів
   *[other] { $count } субтитру
}
text-view-wrap = Переносити рядки
text-view-show-source = Показати джерело
text-view-show-rendered = Показати відформатованим
text-view-text = Текст файлу { $name }
text-view-markdown = Документ Markdown { $name }
text-view-table = Таблиця { $name }
text-view-subtitles = Субтитри файлу { $name }
text-view-picture-cue = Субтитр-зображення
