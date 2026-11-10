## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] שורה אחת
    [two] שתי שורות
   *[other] { $count } שורות
}
text-view-rows = { $rows ->
    [one] שורה אחת
    [two] שתי שורות
   *[other] { $rows } שורות
} × { $columns ->
    [one] עמודה אחת
    [two] שתי עמודות
   *[other] { $columns } עמודות
}
text-view-cues = { $count ->
    [one] כתובית אחת
    [two] שתי כתוביות
   *[other] { $count } כתוביות
}
text-view-wrap = גלישת שורות
text-view-show-source = הצגת המקור
text-view-show-rendered = הצגה מעוצבת
text-view-text = הטקסט של { $name }
text-view-markdown = מסמך Markdown { $name }
text-view-table = טבלה { $name }
text-view-subtitles = הכתוביות של { $name }
text-view-picture-cue = כתובית בתמונה
