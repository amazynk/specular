# Photographer polish pass (2026-11-10).

# Develop: F6 between the picture, the panel and the filmstrip.
palette-develop-next-region = Next region
palette-develop-next-region-help = Move the keyboard on: picture, panel, filmstrip.
palette-develop-prev-region = Previous region
palette-develop-prev-region-help = Move the keyboard back: filmstrip, panel, picture.
develop-region-picture = Picture
develop-region-panel = Panel

# Develop: the Crop card's rows (the crop tool's keys as rows).
develop-crop-aspect-row = Aspect
develop-crop-aspect-option-free = Free
develop-crop-aspect-option-original = Original
develop-crop-aspect-option-custom = Custom
develop-crop-type-ratio = Type a ratio…
develop-crop-swap = Swap portrait and landscape
develop-crop-turn-left = Turn left
develop-crop-turn-right = Turn right
develop-crop-flip-horizontal = Flip left to right
develop-crop-flip-vertical = Flip top to bottom
develop-crop-ratio-title = Crop aspect
develop-crop-ratio-field = Width:height
develop-crop-ratio-ok = Set
develop-crop-ratio-bad = Type a ratio such as 5:4, or one number such as 1.85.

# Import: what each copied photo gets.
import-template = Metadata template
import-style = Develop style
import-choice-none = None
# A choice row stepped, read out: "Metadata template: Copyright".
import-choice-said = { $label }: { $value }
import-keywords = Keywords
import-keywords-hint = Such as Family; Places > Paris
import-style-missing = The style { $name } is gone. The photos were imported without it.

# Keywords: rename, move and merge.
keywords-rename = Rename…
keywords-move = Move to…
keywords-rename-of = New name for { $keyword }
keywords-move-of = Put { $keyword } under (empty: the top)
keywords-move-into-itself = { $keyword } can't go under itself.
keywords-renaming = Renaming { $from } to { $to }…
keywords-merging = Merging { $from } into { $to }…
keywords-renamed = { $from } is now { $to } on { $count ->
    [one] { $count } photo
   *[other] { $count } photos
}
keywords-rename-row = Keyword { $from } to { $to }
keywords-rename-finding = Finding photos…
keywords-rename-row-detail = { $done } of { $total } photos

# Develop: the before/after divider dragged ("Divider at 30 %").
develop-split-moved = Divider at { PERCENT($percent) }

# Develop: auto-apply rules, changed row by row.
develop-rules-placeholder-edit = Choose a rule to change or delete it
develop-rule-edit-placeholder = { $rule }
develop-rule-maker = Maker: { $value }
develop-rule-model = Model: { $value }
develop-rule-lens = Lens: { $value }
develop-rule-iso = ISO: { $value }
develop-rule-type = File type: { $value }
develop-rule-style = Style: { $value }
develop-rule-any = any
develop-rule-type-any = any file
develop-rule-delete = Delete rule…
develop-rule-field-title = Change “{ $rule }”
develop-rule-field-maker = Maker (part of the name is enough; empty: any)
develop-rule-field-model = Model (part of the name is enough; empty: any)
develop-rule-field-lens = Lens (part of the name is enough; empty: any)
develop-rule-field-iso = ISO, such as 100-800, 1600- or -400 (empty: any)
develop-rule-field-ok = Set
develop-rule-iso-bad = Type an ISO range such as 100-800.
develop-rule-style-placeholder = Choose the rule's style
