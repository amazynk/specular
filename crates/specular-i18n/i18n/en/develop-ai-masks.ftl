# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = Subject
develop-mask-kind-sky = Sky
develop-mask-kind-object = Object at click
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] Finding the subject…
        [sky] Finding the sky…
       *[object] Finding the object…
    }
develop-ai-ready = Ready
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = Points: { $n }
develop-ai-click-help =
    Click the object.
    Alt+click what is not part of it.
    Drag for a box.
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    Needs the { $model } add-on.
    Masks nothing until then.
develop-ai-failed = Could not make this mask: { $error }
develop-ai-failed-worker = The AI masks worker stopped.
develop-ai-open-settings = Open Local AI…
develop-ai-clear = Clear points
develop-ai-too-many = At most { $n } points (a box counts as two).
develop-ai-box-set = Box set
develop-ai-point-added = Point added
develop-ai-point-removed = Point added outside the object
develop-ai-cleared = Points cleared
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = Point { $n }, on the object
develop-ai-point-off = Point { $n }, off the object

## Settings › Local AI: the Masks feature
settings-ai-masks = AI masks
settings-ai-masks-help = Subject, sky and object masks in Develop. The object model comes with this feature; subject and sky are add-ons.
settings-ai-masks-size = Model
settings-ai-masks-size-help = EfficientSAM Ti is faster. EfficientSAM S follows edges more closely.
settings-ai-masks-size-recommended = Default
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = Large
