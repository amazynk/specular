# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Swedish. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = Lokal AI
settings-ai-pack = Lokalt AI-paket
# machine-drafted, needs native review
settings-ai-pack-help = Valfria funktioner som bara körs på den här datorn. Inget de hittar lämnar den. Välj en modell för varje funktion; inget laddas ned förrän du väljer det.
# machine-drafted, needs native review
settings-ai-install-from-file = Installera från fil…
settings-ai-install-from-file-help = Installera modellfiler som du har kopierat till den här datorn, för datorer utan internet. Varje fil kontrolleras mot listan som följer med { -app-name }.
settings-ai-install-from-folder = Installera från mapp…
settings-ai-install-from-folder-help = Samma sak för varje modellfil i en mapp.
settings-ai-search = Sök efter beskrivning
settings-ai-search-help = Hitta foton genom att beskriva dem, till exempel ”hund på en strand”, och gruppera liknande bilder bättre.
settings-ai-text = Text i bilder
settings-ai-text-help = Läs texten i foton och skanningar så att sökning med text: hittar dem.
settings-ai-culling = Hjälp med gallring
# machine-drafted, needs native review
settings-ai-culling-help = Markera slutna ögon, oskärpa och dålig exponering för att hjälpa dig välja den bästa bilden i en serie.
settings-ai-people = Personer
# machine-drafted, needs native review
settings-ai-people-help = Hitta ansikten och gruppera dem per person, för sökning med people:. Gruppering är av tills du slår på den för ett bibliotek. Ansiktsdata kan raderas för en person eller för alla på en gång.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] Inga modeller finns i den här versionen.
        [installed] Installerat
        [indexing] Installerat · { $done } av { $total } bilder indexerade
        [downloading] Laddar ned… { $percent }
        [failed] Nedladdningen misslyckades
       *[available] Inte installerat
    }
ai-removing = Tar bort…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Ladda ned
        [cancel] Avbryt
        [remove] Ta bort
        [forget] Glöm
        [faces] Radera alla ansiktsdata
       *[pack] Ta bort paket
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Radera alla ansiktsdata? Alla hittade ansikten och personer raderas. Dina foton finns kvar.
       *[pack] Ta bort det lokala AI-paketet, dess modeller och allt det har hittat? Dina foton ändras inte.
    }

## Gallery search
gallery-query-needs-ai = { $field }: kräver det lokala AI-paketet (Inställningar › Lokal AI)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: det lokala AI-paketet kan inte söka på detta än

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Ansikten visas
# machine-drafted, needs native review
faces-hidden = Ansikten dolda
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Slutna ögon
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Slutna ögon
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Slutna ögon: { $closed } av { $faces } ansikten
# machine-drafted, needs native review
cull-flag-blurred = Oskarp
# machine-drafted, needs native review
cull-flag-dark = För mörk
# machine-drafted, needs native review
cull-flag-bright = För ljus
# machine-drafted, needs native review
cull-flag-clipped = Klippta högdagrar eller skuggor
# machine-drafted, needs native review
# The People view.
people-title = Personer
# machine-drafted, needs native review
people-open = Personer
# machine-drafted, needs native review
people-done = Klar
# machine-drafted, needs native review
people-back = Tillbaka
# machine-drafted, needs native review
people-show-photos = Visa foton
# machine-drafted, needs native review
people-name = Namnge…
# machine-drafted, needs native review
people-name-prompt = Namnge den här personen
# machine-drafted, needs native review
people-faces-open = Ansikten
# machine-drafted, needs native review
people-faces-of = Ansikten för { $name }
# machine-drafted, needs native review
people-merge = Slå ihop
# machine-drafted, needs native review
people-merged = { $count ->
        [one] { $count } person sammanslagen
       *[other] { $count } personer sammanslagna
    }
# machine-drafted, needs native review
people-merge-ask = Slå ihop { $from } med { $into }?
# machine-drafted, needs native review
people-merge-ask-many = Slå ihop { $count } personer med { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = Ansiktena blir { $into }s. Bilderna på disken ändras inte.
# machine-drafted, needs native review
people-not-this = Inte den här personen
# machine-drafted, needs native review
people-face-removed = Ansiktet togs bort från den här personen. Fotot finns kvar.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Person #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } ansikte
       *[other] { $count } ansikten
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } person
       *[other] { $count } personer
    }
# machine-drafted, needs native review
people-face-a11y = Ansikte { $index }
# machine-drafted, needs native review
people-loading = Läser in…
# machine-drafted, needs native review
people-not-installed = Personer är inte installerat. Hämta det under Inställningar › Lokal AI.
# machine-drafted, needs native review
people-grouping-off = Gruppering av ansikten är av för det här biblioteket. Slå på ”Gruppera ansikten till personer” under Inställningar › Lokal AI.
# machine-drafted, needs native review
people-none = Inga personer hittade ännu.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Modell
settings-ai-search-size-help = SigLIP 2 B/16 hittar foton bäst, på många språk. B/32 indexerar ungefär fyra gånger snabbare, för långsammare grafikkort.
settings-ai-search-size-recommended = Standard
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Modell
settings-ai-text-size-help = PP-OCRv6 small läser mer exakt; tiny är snabbare.
settings-ai-text-size-recommended = Standard
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Stor
# machine-drafted, needs native review
settings-ai-culling-size = Modell
settings-ai-culling-size-help = Modellen som hittar slutna ögon.
settings-ai-culling-size-recommended = Standard
settings-ai-culling-size-small = Liten
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Stor
# machine-drafted, needs native review
settings-ai-people-size = Modell
settings-ai-people-size-help = SFace är liten. AuraFace är större och tränad på data som utgivaren säger är licensierade för kommersiell användning.
settings-ai-people-size-recommended = Standard
settings-ai-people-size-small = Liten
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Liten
        [large] Stor
       *[standard] Standard
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Standard ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Standard för ditt { $gpu }: { $size }
ai-recommended = Standard för den här datorn: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Ingen storlek ryms i den här datorns minne.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Körs på: GPU ({ $gpu })
        [gpu-unnamed] Körs på: GPU
        [no-gpu] Körs på: CPU (ingen GPU hittades)
        [differed] Körs på: CPU — GPU:ns resultat skilde sig från CPU:ns
        [failed] Körs på: CPU — modellen misslyckades på GPU:n
        [setting] Körs på: CPU (bearbetningen är inställd på ”Endast CPU”)
        [checking] Körs på: CPU — GPU:n kontrolleras mot den
       *[cpu] Körs på: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } på disken
       *[download] { $bytes } att ladda ned
    } · { $memory } minne{ $gpu ->
        [yes] { " · kräver GPU:n" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Kräver en GPU, och den här datorn har ingen som den kan använda.
       *[memory] Kräver { $need } minne, mer än de { $have } som Specular tillåter för AI på den här datorn.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = { $current } används, inte { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Tillbaka till standard
        [anyway] Använd ändå
       *[use] Använd den här storleken
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Vald men inte nedladdad än: { $current } används fortfarande.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] Används
        [downloaded] Nedladdad, används inte
        [missing] Inte nedladdad
        [previous] Används tills den nya storleken har indexerat ditt bibliotek
        [target] Laddar ned…
        [failed] Nedladdningen misslyckades
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Visa licens
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: byta till { $size }? Ditt bibliotek indexeras om; den nuvarande storleken används tills dess.
       *[other] { $feature }: byta till { $size }? Det här laddar ned { $download } och indexerar om ditt bibliotek; den nuvarande storleken används tills dess.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: använda { $size } ändå? Den kanske inte fungerar på den här datorn, eller bara mycket långsamt.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: { $size } passar nu bättre för den här datorn. Byta? { $download ->
        [none] Ditt bibliotek indexeras om.
       *[other] Det här laddar ned { $download } och indexerar om ditt bibliotek.
    }
ai-suggest-toast = Lokal AI: en annan modellstorlek passar bättre för den här datorn (Inställningar › Lokal AI).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Byter till { $size } · { $done } av { $total } bilder indexerade

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Kommer i en senare version
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Många språk
        [english] Endast engelska
        [cjk-latin] Kinesiska, japanska och latinsk skrift
        [zh-latin] Kinesiska och latinsk skrift
        [arabic] Arabiska
        [hebrew] Hebreiska
        [korean] Koreanska
        [cyrillic] Kyrilliska
        [greek] Grekiska
        [thai] Thailändska
        [devanagari] Devanagari
        [tamil] Tamil
        [telugu] Telugu
        [eslav] Ryska, ukrainska och belarusiska
        [latin] Latinskt alfabet (många europeiska språk)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Snabbare, för långsammare grafikkort
        [closed-eyes] Hittar slutna ögon
        [small-faces] Bättre med små ansikten
        [no-translate] Kan inte översätta till engelska
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Träningsdata: { $source }
        [research] Träningsdata: { $source } (licensierad endast för forskning)
       *[unstated] { $named ->
            [no] Träningsdata: inte namngiven, villkor inte angivna
           *[yes] Träningsdata: { $source }, villkor inte angivna
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Laddas ned från { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Gruppera ansikten till personer
ai-grouping-help = Av tills du slår på det för det här biblioteket. Att slå av stoppar grupperingen; ansikten som redan grupperats finns kvar tills du raderar dem.
ai-grouping-state = { $on ->
        [yes] På i det här biblioteket
       *[no] Av
    }
ai-grouping-button = { $on ->
        [yes] Slå av
       *[no] Slå på
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Ladda ned { $feature } först.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Ta bort { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = Dess modellfiler raderas. Dina foton och den text som redan hittats finns kvar.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Läs text i det här bibliotekets foton
# machine-drafted, needs native review
ai-text-indexing-help = Av tills du slår på det för det här biblioteket. Sedan hittar text: i Bibliotekets sökning foton utifrån orden i dem.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] På i det här biblioteket
       *[no] Av
    }

## Installera från fil

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] En modellfil kopierades. Tryck på ”Ladda ned” för att installera den.
       *[other] { $count } modellfiler kopierades. Tryck på ”Ladda ned” för att installera dem.
    }
ai-sideload-none = Inga modellfiler hittades.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } är ingen modell som { -app-name } känner till. Den kopierades inte.
ai-sideload-failed = Kunde inte kopiera { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Använda en overifierad modell?
ai-unverified-detail = { $file } finns inte på listan över modeller som följer med { -app-name }, så den kan inte kontrolleras. Den används som { $model }.
ai-unverified-use = Använd overifierad modell

# machine-drafted, needs native review
ai-failed-internal = Något gick fel inuti { -app-name }. Loggen har detaljerna.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Byt
       *[keep] Behåll nuvarande storlek
    }
