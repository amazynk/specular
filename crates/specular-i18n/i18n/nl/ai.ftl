# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Dutch. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = Lokale AI
settings-ai-pack = Lokaal AI-pakket
# machine-drafted, needs native review
settings-ai-pack-help = Optionele functies die alleen op deze computer draaien. Niets wat ze vinden verlaat hem. Kies per functie een model; er wordt niets gedownload totdat je het kiest.
# machine-drafted, needs native review
settings-ai-install-from-file = Installeren uit bestand…
settings-ai-install-from-file-help = Installeer modelbestanden die u naar deze computer hebt gekopieerd, voor computers zonder internet. Elk bestand wordt gecontroleerd aan de hand van de lijst die met { -app-name } wordt meegeleverd.
settings-ai-install-from-folder = Installeren uit map…
settings-ai-install-from-folder-help = Hetzelfde, voor elk modelbestand in een map.
settings-ai-search = Zoeken op beschrijving
settings-ai-search-help = Vind foto’s door ze te beschrijven, zoals ‘hond op een strand’, en groepeer vergelijkbare opnamen beter.
settings-ai-text = Tekst in afbeeldingen
settings-ai-text-help = Lees de tekst in foto’s en scans, zodat zoeken met text: ze vindt.
settings-ai-culling = Hulp bij selecteren
# machine-drafted, needs native review
settings-ai-culling-help = Markeer gesloten ogen, onscherpte en slechte belichting om de beste opname uit een reeks te helpen kiezen.
settings-ai-people = Personen
# machine-drafted, needs native review
settings-ai-people-help = Vind gezichten en groepeer ze per persoon, voor zoeken met people:. Groeperen staat uit totdat je het voor een bibliotheek aanzet. Gezichtsgegevens kunnen voor één persoon of voor iedereen tegelijk worden verwijderd.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] Geen modellen beschikbaar in deze build.
        [installed] Geïnstalleerd
        [indexing] Geïnstalleerd · { $done } van { $total } afbeeldingen geïndexeerd
        [downloading] Downloaden… { $percent }
        [failed] Downloaden mislukt
       *[available] Niet geïnstalleerd
    }
ai-removing = Verwijderen…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Downloaden
        [cancel] Annuleren
        [remove] Verwijderen
        [forget] Vergeten
        [faces] Alle gezichtsgegevens verwijderen
       *[pack] Pakket verwijderen
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Alle gezichtsgegevens verwijderen? Alle gevonden gezichten en personen worden verwijderd. Je foto’s blijven.
       *[pack] Het lokale AI-pakket, de modellen en alles wat het heeft gevonden verwijderen? Je foto’s worden niet gewijzigd.
    }

## Gallery search
gallery-query-needs-ai = { $field }: vereist het lokale AI-pakket (Instellingen › Lokale AI)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: het lokale AI-pakket kan hier nog niet op zoeken

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Gezichten getoond
# machine-drafted, needs native review
faces-hidden = Gezichten verborgen
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Ogen dicht
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Ogen dicht
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Ogen dicht: { $closed } van { $faces } gezichten
# machine-drafted, needs native review
cull-flag-blurred = Onscherp
# machine-drafted, needs native review
cull-flag-dark = Te donker
# machine-drafted, needs native review
cull-flag-bright = Te licht
# machine-drafted, needs native review
cull-flag-clipped = Geclipte hoge lichten of schaduwen
# machine-drafted, needs native review
# The People view.
people-title = Personen
# machine-drafted, needs native review
people-open = Personen
# machine-drafted, needs native review
people-done = Klaar
# machine-drafted, needs native review
people-back = Terug
# machine-drafted, needs native review
people-show-photos = Foto’s tonen
# machine-drafted, needs native review
people-name = Naam geven…
# machine-drafted, needs native review
people-name-prompt = Geef deze persoon een naam
# machine-drafted, needs native review
people-faces-open = Gezichten
# machine-drafted, needs native review
people-faces-of = Gezichten van { $name }
# machine-drafted, needs native review
people-merge = Samenvoegen
# machine-drafted, needs native review
people-merged = { $count ->
        [one] { $count } persoon samengevoegd
       *[other] { $count } personen samengevoegd
    }
# machine-drafted, needs native review
people-merge-ask = { $from } samenvoegen met { $into }?
# machine-drafted, needs native review
people-merge-ask-many = { $count } personen samenvoegen met { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = Hun gezichten worden die van { $into }. Foto’s op schijf veranderen niet.
# machine-drafted, needs native review
people-not-this = Niet deze persoon
# machine-drafted, needs native review
people-face-removed = Gezicht losgemaakt van deze persoon. De foto blijft.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Persoon #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } gezicht
       *[other] { $count } gezichten
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } persoon
       *[other] { $count } personen
    }
# machine-drafted, needs native review
people-face-a11y = Gezicht { $index }
# machine-drafted, needs native review
people-loading = Laden…
# machine-drafted, needs native review
people-not-installed = Personen is niet geïnstalleerd. Download het via Instellingen › Lokale AI.
# machine-drafted, needs native review
people-grouping-off = Gezichten groeperen staat uit voor deze bibliotheek. Zet ‘Gezichten groeperen tot personen’ aan in Instellingen › Lokale AI.
# machine-drafted, needs native review
people-none = Nog geen personen gevonden.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Model
settings-ai-search-size-help = SigLIP 2 B/16 vindt foto’s het best, in veel talen. B/32 indexeert ongeveer vier keer sneller, voor langzamere videokaarten.
settings-ai-search-size-recommended = Standaard
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Model
settings-ai-text-size-help = PP-OCRv6 small leest nauwkeuriger; tiny is sneller.
settings-ai-text-size-recommended = Standaard
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Groot
# machine-drafted, needs native review
settings-ai-culling-size = Model
settings-ai-culling-size-help = Het model dat gesloten ogen vindt.
settings-ai-culling-size-recommended = Standaard
settings-ai-culling-size-small = Klein
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Groot
# machine-drafted, needs native review
settings-ai-people-size = Model
settings-ai-people-size-help = SFace is klein. AuraFace is groter en getraind op gegevens die de uitgever gelicentieerd noemt voor commercieel gebruik.
settings-ai-people-size-recommended = Standaard
settings-ai-people-size-small = Klein
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Klein
        [large] Groot
       *[standard] Standaard
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Standaard ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Standaard voor je { $gpu }: { $size }
ai-recommended = Standaard voor deze computer: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Geen enkele grootte past in het geheugen van deze computer.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Draait op: GPU ({ $gpu })
        [gpu-unnamed] Draait op: GPU
        [no-gpu] Draait op: CPU (geen GPU gevonden)
        [differed] Draait op: CPU — het GPU-resultaat week af van dat van de CPU
        [failed] Draait op: CPU — het model faalde op de GPU
        [setting] Draait op: CPU (verwerking staat op ‘Alleen de CPU’)
        [checking] Draait op: CPU — de GPU wordt ermee vergeleken
       *[cpu] Draait op: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } op schijf
       *[download] { $bytes } downloaden
    } · { $memory } geheugen{ $gpu ->
        [yes] { " · vereist de GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Vereist een GPU, en deze computer heeft er geen die hij kan gebruiken.
       *[memory] Vereist { $need } geheugen, meer dan de { $have } die Specular op deze computer voor AI toestaat.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = { $current } is in gebruik, niet { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Terug naar standaard
        [anyway] Toch gebruiken
       *[use] Deze grootte gebruiken
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Gekozen, maar nog niet gedownload: { $current } blijft in gebruik.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] In gebruik
        [downloaded] Gedownload, niet in gebruik
        [missing] Niet gedownload
        [previous] In gebruik tot de nieuwe grootte je bibliotheek heeft geïndexeerd
        [target] Downloaden…
        [failed] Downloaden mislukt
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Licentie bekijken
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: overschakelen naar { $size }? Je bibliotheek wordt opnieuw geïndexeerd; tot dan blijft de huidige grootte in gebruik.
       *[other] { $feature }: overschakelen naar { $size }? Hiermee wordt { $download } gedownload en je bibliotheek opnieuw geïndexeerd; tot dan blijft de huidige grootte in gebruik.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: { $size } toch gebruiken? Het werkt mogelijk niet op deze computer, of alleen heel traag.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: { $size } past nu beter bij deze computer. Overschakelen? { $download ->
        [none] Je bibliotheek wordt opnieuw geïndexeerd.
       *[other] Hiermee wordt { $download } gedownload en je bibliotheek opnieuw geïndexeerd.
    }
ai-suggest-toast = Lokale AI: een andere modelgrootte past beter bij deze computer (Instellingen › Lokale AI).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Overschakelen naar { $size } · { $done } van { $total } afbeeldingen geïndexeerd

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Komt in een latere versie
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Veel talen
        [english] Alleen Engels
        [cjk-latin] Chinees, Japans en Latijns schrift
        [zh-latin] Chinees en Latijns schrift
        [arabic] Arabisch
        [hebrew] Hebreeuws
        [korean] Koreaans
        [cyrillic] Cyrillisch
        [greek] Grieks
        [thai] Thai
        [devanagari] Devanagari
        [tamil] Tamil
        [telugu] Telugu
        [eslav] Russisch, Oekraïens en Wit-Russisch
        [latin] Latijns schrift (veel Europese talen)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Sneller, voor langzamere videokaarten
        [closed-eyes] Vindt gesloten ogen
        [small-faces] Beter bij kleine gezichten
        [no-translate] Kan niet naar het Engels vertalen
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Trainingsgegevens: { $source }
        [research] Trainingsgegevens: { $source } (alleen gelicentieerd voor onderzoek)
       *[unstated] { $named ->
            [no] Trainingsgegevens: niet genoemd, voorwaarden niet vermeld
           *[yes] Trainingsgegevens: { $source }, voorwaarden niet vermeld
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Wordt gedownload van { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Gezichten groeperen tot personen
ai-grouping-help = Uit totdat je het voor deze bibliotheek aanzet. Uitzetten stopt het groeperen; al gegroepeerde gezichten blijven totdat je ze verwijdert.
ai-grouping-state = { $on ->
        [yes] Aan in deze bibliotheek
       *[no] Uit
    }
ai-grouping-button = { $on ->
        [yes] Uitzetten
       *[no] Aanzetten
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Download eerst { $feature }.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = { $name } verwijderen?
# machine-drafted, needs native review
ai-addon-remove-detail = De modelbestanden worden verwijderd. Je foto’s en de al gevonden tekst blijven.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Tekst in de foto’s van deze bibliotheek lezen
# machine-drafted, needs native review
ai-text-indexing-help = Uit totdat je het voor deze bibliotheek aanzet. Daarna vindt text: in de zoekfunctie van de Bibliotheek foto’s op de woorden erin.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Aan in deze bibliotheek
       *[no] Uit
    }

## Installeren uit bestand

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Eén modelbestand gekopieerd. Klik op ‘Downloaden’ om het te installeren.
       *[other] { $count } modelbestanden gekopieerd. Klik op ‘Downloaden’ om ze te installeren.
    }
ai-sideload-none = Geen modelbestanden gevonden.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } is geen model dat { -app-name } kent. Het is niet gekopieerd.
ai-sideload-failed = Kan { $file } niet kopiëren: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Een niet-gecontroleerd model gebruiken?
ai-unverified-detail = { $file } staat niet op de lijst met modellen die met { -app-name } worden meegeleverd, dus het kan niet worden gecontroleerd. Het wordt gebruikt als { $model }.
ai-unverified-use = Niet-gecontroleerd model gebruiken

# machine-drafted, needs native review
ai-failed-internal = Er is iets misgegaan in { -app-name }. Het logboek bevat de details.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Overschakelen
       *[keep] Huidige grootte houden
    }
