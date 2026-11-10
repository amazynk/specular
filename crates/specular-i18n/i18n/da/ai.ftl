# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Danish. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = Lokal AI
settings-ai-pack = Lokal AI-pakke
# machine-drafted, needs native review
settings-ai-pack-help = Valgfrie funktioner, der kun kører på denne computer. Intet, de finder, forlader den. Vælg en model til hver funktion; intet hentes, før du vælger det.
# machine-drafted, needs native review
settings-ai-install-from-file = Installér fra fil…
settings-ai-install-from-file-help = Installér modelfiler, du har kopieret til denne computer, til maskiner uden internet. Hver fil kontrolleres mod listen, der følger med { -app-name }.
settings-ai-install-from-folder = Installér fra mappe…
settings-ai-install-from-folder-help = Det samme for alle modelfiler i en mappe.
settings-ai-search = Søg efter beskrivelse
settings-ai-search-help = Find fotos ved at beskrive dem, fx »hund på en strand«, og gruppér lignende billeder bedre.
settings-ai-text = Tekst i billeder
settings-ai-text-help = Læs teksten i fotos og scanninger, så søgning med text: finder dem.
settings-ai-culling = Hjælp til sortering
# machine-drafted, needs native review
settings-ai-culling-help = Markér lukkede øjne, uskarphed og dårlig eksponering for at hjælpe med at vælge det bedste billede i en serie.
settings-ai-people = Personer
# machine-drafted, needs native review
settings-ai-people-help = Find ansigter og gruppér dem efter person til søgning med people:. Gruppering er slået fra, indtil du slår den til for et bibliotek. Ansigtsdata kan slettes for én person eller for alle på én gang.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] Ingen modeller tilgængelige i denne version.
        [installed] Installeret
        [indexing] Installeret · { $done } af { $total } billeder indekseret
        [downloading] Henter… { $percent }
        [failed] Download mislykkedes
       *[available] Ikke installeret
    }
ai-removing = Fjerner…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Hent
        [cancel] Annuller
        [remove] Fjern
        [forget] Glem
        [faces] Slet alle ansigtsdata
       *[pack] Fjern pakke
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Slet alle ansigtsdata? Alle fundne ansigter og personer slettes. Dine fotos bevares.
       *[pack] Fjern den lokale AI-pakke, dens modeller og alt, hvad den har fundet? Dine fotos ændres ikke.
    }

## Gallery search
gallery-query-needs-ai = { $field }: kræver den lokale AI-pakke (Indstillinger › Lokal AI)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: den lokale AI-pakke kan ikke søge i dette endnu

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Ansigter vises
# machine-drafted, needs native review
faces-hidden = Ansigter skjult
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Lukkede øjne
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Lukkede øjne
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Lukkede øjne: { $closed } af { $faces } ansigter
# machine-drafted, needs native review
cull-flag-blurred = Uskarpt
# machine-drafted, needs native review
cull-flag-dark = For mørkt
# machine-drafted, needs native review
cull-flag-bright = For lyst
# machine-drafted, needs native review
cull-flag-clipped = Klippede højlys eller skygger
# machine-drafted, needs native review
# The People view.
people-title = Personer
# machine-drafted, needs native review
people-open = Personer
# machine-drafted, needs native review
people-done = Færdig
# machine-drafted, needs native review
people-back = Tilbage
# machine-drafted, needs native review
people-show-photos = Vis fotos
# machine-drafted, needs native review
people-name = Navngiv…
# machine-drafted, needs native review
people-name-prompt = Navngiv denne person
# machine-drafted, needs native review
people-faces-open = Ansigter
# machine-drafted, needs native review
people-faces-of = Ansigter for { $name }
# machine-drafted, needs native review
people-merge = Flet
# machine-drafted, needs native review
people-merged = { $count ->
        [one] { $count } person flettet
       *[other] { $count } personer flettet
    }
# machine-drafted, needs native review
people-merge-ask = Flet { $from } ind i { $into }?
# machine-drafted, needs native review
people-merge-ask-many = Flet { $count } personer ind i { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = Ansigterne bliver { $into }s. Fotos på disken ændres ikke.
# machine-drafted, needs native review
people-not-this = Ikke denne person
# machine-drafted, needs native review
people-face-removed = Ansigtet er fjernet fra denne person. Billedet bevares.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Person #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } ansigt
       *[other] { $count } ansigter
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } person
       *[other] { $count } personer
    }
# machine-drafted, needs native review
people-face-a11y = Ansigt { $index }
# machine-drafted, needs native review
people-loading = Indlæser…
# machine-drafted, needs native review
people-not-installed = Personer er ikke installeret. Hent det under Indstillinger › Lokal AI.
# machine-drafted, needs native review
people-grouping-off = Gruppering af ansigter er slået fra for dette bibliotek. Slå »Gruppér ansigter i personer« til under Indstillinger › Lokal AI.
# machine-drafted, needs native review
people-none = Ingen personer fundet endnu.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Model
settings-ai-search-size-help = SigLIP 2 B/16 finder fotos bedst, på mange sprog. B/32 indekserer omkring fire gange hurtigere, til langsommere grafikkort.
settings-ai-search-size-recommended = Standard
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Model
settings-ai-text-size-help = PP-OCRv6 small læser mere præcist; tiny er hurtigere.
settings-ai-text-size-recommended = Standard
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Stor
# machine-drafted, needs native review
settings-ai-culling-size = Model
settings-ai-culling-size-help = Modellen, der finder lukkede øjne.
settings-ai-culling-size-recommended = Standard
settings-ai-culling-size-small = Lille
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Stor
# machine-drafted, needs native review
settings-ai-people-size = Model
settings-ai-people-size-help = SFace er lille. AuraFace er større og trænet på data, som udgiveren kalder licenseret til kommerciel brug.
settings-ai-people-size-recommended = Standard
settings-ai-people-size-small = Lille
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Lille
        [large] Stor
       *[standard] Standard
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Standard ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Standard til dit { $gpu }: { $size }
ai-recommended = Standard til denne computer: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Ingen størrelse passer til denne computers hukommelse.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Kører på: GPU ({ $gpu })
        [gpu-unnamed] Kører på: GPU
        [no-gpu] Kører på: CPU (ingen GPU fundet)
        [differed] Kører på: CPU — GPU’ens resultat afveg fra CPU’ens
        [failed] Kører på: CPU — modellen fejlede på GPU’en
        [setting] Kører på: CPU (behandlingen er sat til “Kun CPU”)
        [checking] Kører på: CPU — GPU’en kontrolleres op imod den
       *[cpu] Kører på: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } på disken
       *[download] { $bytes } at hente
    } · { $memory } hukommelse{ $gpu ->
        [yes] { " · kræver GPU’en" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Kræver en GPU, og denne computer har ingen, den kan bruge.
       *[memory] Kræver { $need } hukommelse, mere end de { $have }, som Specular tillader til AI på denne computer.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = { $current } er i brug, ikke { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Tilbage til standard
        [anyway] Brug alligevel
       *[use] Brug denne størrelse
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Valgt, men ikke hentet endnu: { $current } bruges fortsat.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] I brug
        [downloaded] Hentet, ikke i brug
        [missing] Ikke hentet
        [previous] I brug, indtil den nye størrelse har indekseret dit bibliotek
        [target] Henter…
        [failed] Download mislykkedes
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Vis licens
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: skift til { $size }? Dit bibliotek bliver indekseret igen; den nuværende størrelse bruges indtil da.
       *[other] { $feature }: skift til { $size }? Det henter { $download } og indekserer dit bibliotek igen; den nuværende størrelse bruges indtil da.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: brug { $size } alligevel? Den kører måske ikke på denne computer, eller kun meget langsomt.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: { $size } passer nu bedre til denne computer. Skift? { $download ->
        [none] Dit bibliotek bliver indekseret igen.
       *[other] Det henter { $download } og indekserer dit bibliotek igen.
    }
ai-suggest-toast = Lokal AI: en anden modelstørrelse passer bedre til denne computer (Indstillinger › Lokal AI).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Skifter til { $size } · { $done } af { $total } billeder indekseret

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Kommer i en senere version
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Mange sprog
        [english] Kun engelsk
        [cjk-latin] Kinesisk, japansk og latinsk skrift
        [zh-latin] Kinesisk og latinsk skrift
        [arabic] Arabisk
        [hebrew] Hebraisk
        [korean] Koreansk
        [cyrillic] Kyrillisk
        [greek] Græsk
        [thai] Thai
        [devanagari] Devanagari
        [tamil] Tamil
        [telugu] Telugu
        [eslav] Russisk, ukrainsk og belarusisk
        [latin] Latinsk skrift (mange europæiske sprog)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Hurtigere, til langsommere grafikkort
        [closed-eyes] Finder lukkede øjne
        [small-faces] Bedre til små ansigter
        [no-translate] Kan ikke oversætte til engelsk
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Træningsdata: { $source }
        [research] Træningsdata: { $source } (kun licenseret til forskning)
       *[unstated] { $named ->
            [no] Træningsdata: ikke oplyst, vilkår ikke oplyst
           *[yes] Træningsdata: { $source }, vilkår ikke oplyst
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Hentes fra { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Gruppér ansigter i personer
ai-grouping-help = Slået fra, indtil du slår det til for dette bibliotek. Slår du det fra, stopper grupperingen; ansigter, der allerede er grupperet, bliver, indtil du sletter dem.
ai-grouping-state = { $on ->
        [yes] Slået til i dette bibliotek
       *[no] Slået fra
    }
ai-grouping-button = { $on ->
        [yes] Slå fra
       *[no] Slå til
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Hent { $feature } først.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Fjern { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = Dens modelfiler slettes. Dine fotos og den tekst, der allerede er fundet, bevares.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Læs tekst i dette biblioteks fotos
# machine-drafted, needs native review
ai-text-indexing-help = Slået fra, indtil du slår det til for dette bibliotek. Derefter finder text: i Bibliotekets søgning fotos ud fra ordene i dem.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Slået til i dette bibliotek
       *[no] Slået fra
    }

## Installér fra fil

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Kopierede én modelfil. Tryk på “Hent” for at installere den.
       *[other] Kopierede { $count } modelfiler. Tryk på “Hent” for at installere dem.
    }
ai-sideload-none = Ingen modelfiler fundet.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } er ikke en model, { -app-name } kender. Den blev ikke kopieret.
ai-sideload-failed = Kunne ikke kopiere { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Brug en ubekræftet model?
ai-unverified-detail = { $file } er ikke på listen over modeller, der følger med { -app-name }, så den kan ikke kontrolleres. Den bliver brugt som { $model }.
ai-unverified-use = Brug ubekræftet model

# machine-drafted, needs native review
ai-failed-internal = Noget gik galt i { -app-name }. Loggen har detaljerne.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Skift
       *[keep] Behold nuværende størrelse
    }
