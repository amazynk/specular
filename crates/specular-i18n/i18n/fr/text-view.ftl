## Text view: plain text, logs, Markdown sources, tables and subtitle files.

text-view-lines = { $count ->
    [one] { $count } ligne
   *[other] { $count } lignes
}
text-view-rows = { $rows ->
    [one] { $rows } ligne
   *[other] { $rows } lignes
} × { $columns ->
    [one] { $columns } colonne
   *[other] { $columns } colonnes
}
text-view-cues = { $count ->
    [one] { $count } sous-titre
   *[other] { $count } sous-titres
}
text-view-wrap = Renvoyer à la ligne
text-view-show-source = Afficher la source
text-view-show-rendered = Afficher la mise en forme
text-view-text = Texte de { $name }
text-view-markdown = Document Markdown { $name }
text-view-table = Tableau { $name }
text-view-subtitles = Sous-titres de { $name }
text-view-picture-cue = Sous-titre en image
