# machine-drafted, needs native review
# Photographer polish pass: Develop regions, Crop card rows, import choices, keyword rename and move, auto-apply rules.

# Develop: F6 between the picture, the panel and the filmstrip.
palette-develop-next-region = Nästa område
palette-develop-next-region-help = Flytta tangentbordet framåt: bild, panel, filmremsa.
palette-develop-prev-region = Föregående område
palette-develop-prev-region-help = Flytta tangentbordet bakåt: filmremsa, panel, bild.
develop-region-picture = Bild
develop-region-panel = Panelen

# Develop: the Crop card's rows (the crop tool's keys as rows).
develop-crop-aspect-row = Format
develop-crop-aspect-option-free = Fritt
develop-crop-aspect-option-original = Original
develop-crop-aspect-option-custom = Eget
develop-crop-type-ratio = Skriv ett förhållande…
develop-crop-swap = Byt mellan stående och liggande
develop-crop-turn-left = Rotera åt vänster
develop-crop-turn-right = Rotera åt höger
develop-crop-flip-horizontal = Spegelvänd från vänster till höger
develop-crop-flip-vertical = Spegelvänd uppifrån och ned
develop-crop-ratio-title = Beskärningsformat
develop-crop-ratio-field = Bredd:höjd
develop-crop-ratio-ok = Ange
develop-crop-ratio-bad = Skriv ett förhållande som 5:4, eller ett tal som 1.85.

# Import: what each copied photo gets.
import-template = Metadatamall
import-style = Framkallningsstil
import-choice-none = Ingen
# A choice row stepped, read out: "Metadata template: Copyright".
import-choice-said = { $label }: { $value }
import-keywords = Nyckelord
import-keywords-hint = Till exempel Familj; Platser > Paris
import-style-missing = Stilen { $name } finns inte längre. Bilderna importerades utan den.

# Keywords: rename, move and merge.
keywords-rename = Byt namn…
keywords-move = Flytta till…
keywords-rename-of = Nytt namn för { $keyword }
keywords-move-of = Lägg { $keyword } under (tomt: översta nivån)
keywords-move-into-itself = { $keyword } kan inte läggas under sig självt.
keywords-renaming = Byter namn på { $from } till { $to }…
keywords-merging = Slår ihop { $from } med { $to }…
keywords-renamed = { $from } heter nu { $to } på { $count ->
    [one] { $count } bild
   *[other] { $count } bilder
}
keywords-rename-row = Nyckelordet { $from } till { $to }
keywords-rename-finding = Letar efter bilder…
keywords-rename-row-detail = { $done } av { $total } bilder

# Develop: the before/after divider dragged ("Divider at 30 %").
develop-split-moved = Delaren vid { PERCENT($percent) }

# Develop: auto-apply rules, changed row by row.
develop-rules-placeholder-edit = Välj en regel för att ändra eller ta bort den
develop-rule-edit-placeholder = { $rule }
develop-rule-maker = Tillverkare: { $value }
develop-rule-model = Modell: { $value }
develop-rule-lens = Objektiv: { $value }
develop-rule-iso = ISO: { $value }
develop-rule-type = Filtyp: { $value }
develop-rule-style = Stil: { $value }
develop-rule-any = vilken som helst
develop-rule-type-any = vilken fil som helst
develop-rule-delete = Ta bort regel…
develop-rule-field-title = Ändra ”{ $rule }”
develop-rule-field-maker = Tillverkare (en del av namnet räcker; tomt: vilken som helst)
develop-rule-field-model = Modell (en del av namnet räcker; tomt: vilken som helst)
develop-rule-field-lens = Objektiv (en del av namnet räcker; tomt: vilket som helst)
develop-rule-field-iso = ISO, till exempel 100-800, 1600- eller -400 (tomt: vilket som helst)
develop-rule-field-ok = Ange
develop-rule-iso-bad = Skriv ett ISO-intervall som 100-800.
develop-rule-style-placeholder = Välj regelns stil
