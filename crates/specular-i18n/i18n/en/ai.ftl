# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). English.

## Settings → Local AI
settings-section-ai = Local AI
settings-ai-pack = Local AI pack
settings-ai-pack-help = Optional features that run only on this computer. Nothing they find leaves it. Pick a model for each feature; nothing downloads until you choose it.
settings-ai-install-from-file = Install from file…
settings-ai-install-from-file-help = Install model files you copied to this computer, for machines without internet. Each file is checked against the list shipped with { -app-name }.
settings-ai-install-from-folder = Install from folder…
settings-ai-install-from-folder-help = The same, for every model file in a folder.
settings-ai-search = Search by description
settings-ai-search-help = Find photos by describing them, such as “dog on a beach”, and group similar shots better.
settings-ai-text = Text in images
settings-ai-text-help = Read the text in photos and scans, so the text: search finds them.
settings-ai-culling = Culling assistance
settings-ai-culling-help = Flag closed eyes, blur and poor exposure to help pick the best shot of a burst.
settings-ai-people = People
settings-ai-people-help = Find faces and group them by person, for the people: search. Grouping is off until you turn it on for a library. Face data can be deleted for one person or all at once.
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Model
settings-ai-search-size-help = SigLIP 2 B/16 finds photos best, in many languages. B/32 indexes about four times faster, for slower graphics cards.
settings-ai-search-size-recommended = Default
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Model
settings-ai-text-size-help = The small model reads more accurately; the tiny one is faster.
settings-ai-text-size-recommended = Default
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Large
settings-ai-culling-size = Model
settings-ai-culling-size-help = The model that finds closed eyes.
settings-ai-culling-size-recommended = Default
settings-ai-culling-size-small = Small
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Large
settings-ai-people-size = Model
settings-ai-people-size-help = SFace is small. AuraFace is larger, trained on data its publisher calls licensed for commercial use.
settings-ai-people-size-recommended = Default
settings-ai-people-size-small = Small
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace

# A model size by name.
ai-tier = { $tier ->
        [small] Small
        [large] Large
       *[standard] Standard
    }
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Default ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Default for your { $gpu }: { $size }
ai-recommended = Default for this computer: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = No size fits this computer's memory.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Runs on: GPU ({ $gpu })
        [gpu-unnamed] Runs on: GPU
        [no-gpu] Runs on: CPU (no GPU found)
        [differed] Runs on: CPU — the GPU result differed from the CPU's
        [failed] Runs on: CPU — the model failed on the GPU
        [setting] Runs on: CPU (processing is set to “CPU only”)
        [checking] Runs on: CPU — checking the GPU against it
       *[cpu] Runs on: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } on disk
       *[download] { $bytes } download
    } · { $memory } memory{ $gpu ->
        [yes] { " · needs the GPU" }
       *[no] { "" }
    }
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Needs a GPU, and this computer has none it can use.
       *[memory] Needs { $need } of memory, more than the { $have } Specular allows for AI on this computer.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = { $current } is in use, not { $chosen }.
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Back to default
        [anyway] Use anyway
       *[use] Use this size
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Chosen, but not downloaded yet: { $current } stays in use.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] In use
        [downloaded] Downloaded, not in use
        [missing] Not downloaded
        [previous] In use until the new size has indexed your library
        [target] Downloading…
        [failed] Download failed
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = View licence
# A size change that downloads and re-indexes: asked first. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] Switch { $feature } to { $size }? Your library is indexed again; the current size stays in use until then.
       *[other] Switch { $feature } to { $size }? This downloads { $download } and indexes your library again; the current size stays in use until then.
    }
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = Use { $size } for { $feature } anyway? It may not run on this computer, or only very slowly.
# The recommendation moved (the first calibration, another GPU): offered, never applied
# silently, and only while the size is left on Recommended.
ai-suggest-question = { $size } now suits this computer better for { $feature }. Switch? { $download ->
        [none] Your library is indexed again.
       *[other] This downloads { $download } and indexes your library again.
    }
ai-suggest-toast = Local AI: another model size suits this computer better (Settings › Local AI).

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] No models available in this build.
        [installed] Installed
        [indexing] Installed · { $done } of { $total } images indexed
        [downloading] Downloading… { $percent }
        [failed] Download failed
       *[available] Not installed
    }
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Switching to { $size } · { $done } of { $total } images indexed
## The model manager: what each model says besides its size and licence
# A model or feature not installable yet.
ai-coming = Coming in a later version
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Many languages
        [english] English only
        [cjk-latin] Chinese, Japanese and Latin script
        [zh-latin] Chinese and Latin script
        [arabic] Arabic
        [hebrew] Hebrew
        [korean] Korean
        [cyrillic] Cyrillic
        [greek] Greek
        [thai] Thai
        [devanagari] Devanagari
        [tamil] Tamil
        [telugu] Telugu
        [eslav] Russian, Ukrainian and Belarusian
        [latin] Latin script (many European languages)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces, no-translate.
ai-choice-hint = { $hint ->
        [faster] Faster, for slower graphics cards
        [closed-eyes] Finds closed eyes
        [small-faces] Better with small faces
        [no-translate] Can't translate to English
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Training data: { $source }
        [research] Training data: { $source } (licensed for research only)
       *[unstated] { $named ->
            [no] Training data: not named, terms not stated
           *[yes] Training data: { $source }, terms not stated
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Downloads from { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Group faces into people
ai-grouping-help = Off until you turn it on for this library. Turning it off stops grouping; faces already grouped stay until you delete them.
ai-grouping-state = { $on ->
        [yes] On in this library
       *[no] Off
    }
ai-grouping-button = { $on ->
        [yes] Turn off
       *[no] Turn on
    }
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Download { $feature } first.
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Remove { $name }?
ai-addon-remove-detail = Its model files are deleted. Your photos and the text already found stay.
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Read text in this library’s photos
ai-text-indexing-help = Off until you turn it on for this library. Then text: in the Library’s search finds photos by the words in them.
ai-text-indexing-state = { $on ->
        [yes] On in this library
       *[no] Off
    }

## Install from file (sideloading)

# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Copied one model file. Press Download to install it.
       *[other] Copied { $count } model files. Press Download to install them.
    }
ai-sideload-none = No model files found.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } isn’t a model { -app-name } knows. It wasn’t copied.
ai-sideload-failed = Couldn’t copy { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Use an unverified model?
ai-unverified-detail = { $file } isn’t on the list of models shipped with { -app-name }, so it can’t be checked. It will be used as { $model }.
ai-unverified-use = Use unverified model

# Why a download failed, under "Download failed", when it stopped inside the app.
ai-failed-internal = Something went wrong inside { -app-name }. The log has the details.
# A feature (or the whole pack) being removed, in the background.
ai-removing = Removing…
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Download
        [cancel] Cancel
        [remove] Remove
        [forget] Forget
        [faces] Delete all face data
       *[pack] Remove pack
    }
# The buttons under a model-size question: switch to the size asked about, or keep the one in
# use. "Switch" is also the button of a downloaded size not in use. (A message of their own,
# so a locale without them falls back to these words.)
ai-size-button = { $action ->
        [switch] Switch
       *[keep] Keep current size
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Delete all face data? Every face and person found is deleted. Your photos stay.
       *[pack] Remove the Local AI pack, its models and everything it found? Your photos are not changed.
    }

## Gallery search
gallery-query-needs-ai = { $field }: needs the Local AI pack (Settings › Local AI)
gallery-query-ai-not-yet = { $field }: the Local AI pack can’t search this yet

## Faces: People view, face boxes and culling flags
# "Show faces" on the picture.
faces-shown = Faces shown
faces-hidden = Faces hidden
# Beside a face box whose eyes look closed.
faces-eyes-closed = Eyes closed
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Eyes closed
cull-flag-eyes-closed-some = Eyes closed: { $closed } of { $faces } faces
cull-flag-blurred = Blurred
cull-flag-dark = Dark
cull-flag-bright = Bright
cull-flag-clipped = Clipped highlights or shadows
# The People view.
people-title = People
people-open = People
people-done = Done
people-back = Back
people-show-photos = Show photos
people-name = Name…
people-name-prompt = Name this person
people-faces-open = Faces
people-faces-of = Faces of { $name }
people-merge = Merge
people-merged = { $count ->
        [one] { $count } person merged
       *[other] { $count } people merged
    }
# A drag dropped one person onto another; $from and $into are their names.
people-merge-ask = Merge { $from } into { $into }?
# M with several people selected; $count is how many merge into $into.
people-merge-ask-many = Merge { $count } people into { $into }?
people-merge-ask-detail = Their faces become { $into }’s. Photos on disk don’t change.
people-not-this = Not this person
people-face-removed = Face taken from this person. The photo stays.
# Someone not named yet.
people-unnamed = Person #{ $id }
people-face-count = { $count ->
        [one] { $count } face
       *[other] { $count } faces
    }
people-count = { $count ->
        [one] { $count } person
       *[other] { $count } people
    }
people-face-a11y = Face { $index }
people-loading = Loading…
people-not-installed = People is not installed. Download it in Settings › Local AI.
people-grouping-off = Grouping faces is off for this library. Turn on “Group faces into people” in Settings › Local AI.
people-none = No people found yet.
