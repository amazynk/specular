## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [zero] لا أسطر
    [one] سطر واحد
    [two] سطران
    [few] { $count } أسطر
    [many] { $count } سطرًا
   *[other] { $count } سطر
}
text-view-rows = { $rows ->
    [zero] لا صفوف
    [one] صف واحد
    [two] صفان
    [few] { $rows } صفوف
    [many] { $rows } صفًا
   *[other] { $rows } صف
} × { $columns ->
    [zero] لا أعمدة
    [one] عمود واحد
    [two] عمودان
    [few] { $columns } أعمدة
    [many] { $columns } عمودًا
   *[other] { $columns } عمود
}
text-view-cues = { $count ->
    [zero] لا ترجمات
    [one] ترجمة واحدة
    [two] ترجمتان
    [few] { $count } ترجمات
    [many] { $count } ترجمة
   *[other] { $count } ترجمة
}
text-view-wrap = التفاف الأسطر
text-view-show-source = عرض المصدر
text-view-show-rendered = عرض منسّق
text-view-text = نص { $name }
text-view-markdown = مستند Markdown { $name }
text-view-table = جدول { $name }
text-view-subtitles = ترجمات { $name }
text-view-picture-cue = ترجمة مصوّرة
