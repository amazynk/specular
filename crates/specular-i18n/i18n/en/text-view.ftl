## Text view: plain text, logs, Markdown sources, tables and subtitle files.

# The bar above the text: how many lines the file has.
text-view-lines = { $count ->
    [one] { $count } line
   *[other] { $count } lines
}
# The bar above a table.
text-view-rows = { $rows ->
    [one] { $rows } row
   *[other] { $rows } rows
} × { $columns ->
    [one] { $columns } column
   *[other] { $columns } columns
}
# The bar above a subtitle file's cues.
text-view-cues = { $count ->
    [one] { $count } cue
   *[other] { $count } cues
}
# Toggle: long lines continue on the next line, or run off to the side.
text-view-wrap = Wrap lines
# Markdown: switch to the file's text, or back to the formatted pages.
text-view-show-source = Show source
text-view-show-rendered = Show formatted
# Screen reader names of the view.
text-view-text = Text of { $name }
text-view-markdown = Markdown document { $name }
text-view-table = Table { $name }
text-view-subtitles = Subtitles of { $name }
# Screen reader text for a subtitle that is a picture.
text-view-picture-cue = Picture subtitle
