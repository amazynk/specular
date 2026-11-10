# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Italian. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = IA locale
settings-ai-pack = Pacchetto IA locale
# machine-drafted, needs native review
settings-ai-pack-help = Funzioni facoltative che funzionano solo su questo computer. Nulla di ciò che trovano lo lascia. Scegli un modello per ogni funzione; non viene scaricato nulla finché non lo scegli.
# machine-drafted, needs native review
settings-ai-install-from-file = Installa da file…
settings-ai-install-from-file-help = Installa file di modello copiati su questo computer, per le macchine senza internet. Ogni file viene verificato con l’elenco fornito con { -app-name }.
settings-ai-install-from-folder = Installa da cartella…
settings-ai-install-from-folder-help = Lo stesso, per ogni file di modello in una cartella.
settings-ai-search = Ricerca per descrizione
settings-ai-search-help = Trova le foto descrivendole, ad esempio «cane in spiaggia», e raggruppa meglio gli scatti simili.
settings-ai-text = Testo nelle immagini
settings-ai-text-help = Legge il testo in foto e scansioni, così la ricerca text: le trova.
settings-ai-culling = Aiuto alla selezione
# machine-drafted, needs native review
settings-ai-culling-help = Segnala occhi chiusi, sfocatura ed esposizione scorretta per aiutare a scegliere lo scatto migliore di una raffica.
settings-ai-people = Persone
# machine-drafted, needs native review
settings-ai-people-help = Trova i volti e li raggruppa per persona, per la ricerca people:. Il raggruppamento resta disattivato finché non lo attivi per una libreria. I dati dei volti si possono eliminare per una persona o per tutte insieme.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] Nessun modello disponibile in questa build.
        [installed] Installato
        [indexing] Installato · { $done } di { $total } immagini indicizzate
        [downloading] Download… { $percent }
        [failed] Download non riuscito
       *[available] Non installato
    }
ai-removing = Rimozione…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Scarica
        [cancel] Annulla
        [remove] Rimuovi
        [forget] Dimentica
        [faces] Elimina tutti i dati dei volti
       *[pack] Rimuovi pacchetto
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Eliminare tutti i dati dei volti? Vengono eliminati tutti i volti e le persone trovati. Le tue foto restano.
       *[pack] Rimuovere il pacchetto IA locale, i suoi modelli e tutto ciò che ha trovato? Le tue foto non vengono modificate.
    }

## Gallery search
gallery-query-needs-ai = { $field }: richiede il pacchetto IA locale (Impostazioni › IA locale)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: il pacchetto IA locale non può ancora cercare questo

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Volti mostrati
# machine-drafted, needs native review
faces-hidden = Volti nascosti
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Occhi chiusi
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Occhi chiusi
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Occhi chiusi: { $closed } su { $faces } volti
# machine-drafted, needs native review
cull-flag-blurred = Sfocata
# machine-drafted, needs native review
cull-flag-dark = Troppo scura
# machine-drafted, needs native review
cull-flag-bright = Troppo chiara
# machine-drafted, needs native review
cull-flag-clipped = Luci o ombre tagliate
# machine-drafted, needs native review
# The People view.
people-title = Persone
# machine-drafted, needs native review
people-open = Persone
# machine-drafted, needs native review
people-done = Fatto
# machine-drafted, needs native review
people-back = Indietro
# machine-drafted, needs native review
people-show-photos = Mostra foto
# machine-drafted, needs native review
people-name = Assegna nome…
# machine-drafted, needs native review
people-name-prompt = Dai un nome a questa persona
# machine-drafted, needs native review
people-faces-open = Volti
# machine-drafted, needs native review
people-faces-of = Volti di { $name }
# machine-drafted, needs native review
people-merge = Unisci
# machine-drafted, needs native review
people-merged = { $count ->
        [one] { $count } persona unita
       *[other] { $count } persone unite
    }
# machine-drafted, needs native review
people-merge-ask = Unire { $from } a { $into }?
# machine-drafted, needs native review
people-merge-ask-many = Unire { $count } persone in { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = I suoi volti diventano di { $into }. Le foto sul disco non cambiano.
# machine-drafted, needs native review
people-not-this = Non è questa persona
# machine-drafted, needs native review
people-face-removed = Volto tolto da questa persona. La foto resta.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Persona #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } volto
       *[other] { $count } volti
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } persona
       *[other] { $count } persone
    }
# machine-drafted, needs native review
people-face-a11y = Volto { $index }
# machine-drafted, needs native review
people-loading = Caricamento…
# machine-drafted, needs native review
people-not-installed = Persone non è installato. Scaricalo in Impostazioni › IA locale.
# machine-drafted, needs native review
people-grouping-off = Il raggruppamento dei volti è disattivato per questa libreria. Attiva «Raggruppa i volti in persone» in Impostazioni › IA locale.
# machine-drafted, needs native review
people-none = Nessuna persona trovata finora.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Modello
settings-ai-search-size-help = SigLIP 2 B/16 trova meglio le foto, in molte lingue. B/32 indicizza circa quattro volte più velocemente, per schede grafiche più lente.
settings-ai-search-size-recommended = Predefinito
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Modello
settings-ai-text-size-help = PP-OCRv6 small legge con più precisione; tiny è più veloce.
settings-ai-text-size-recommended = Predefinito
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Grande
# machine-drafted, needs native review
settings-ai-culling-size = Modello
settings-ai-culling-size-help = Il modello che rileva gli occhi chiusi.
settings-ai-culling-size-recommended = Predefinito
settings-ai-culling-size-small = Piccolo
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Grande
# machine-drafted, needs native review
settings-ai-people-size = Modello
settings-ai-people-size-help = SFace è piccolo. AuraFace è più grande, addestrato su dati che il suo editore dichiara concessi in licenza per uso commerciale.
settings-ai-people-size-recommended = Predefinito
settings-ai-people-size-small = Piccolo
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Piccolo
        [large] Grande
       *[standard] Standard
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Predefinito ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Predefinito per la tua { $gpu }: { $size }
ai-recommended = Predefinito per questo computer: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Nessuna dimensione rientra nella memoria di questo computer.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Esecuzione su: GPU ({ $gpu })
        [gpu-unnamed] Esecuzione su: GPU
        [no-gpu] Esecuzione su: CPU (nessuna GPU trovata)
        [differed] Esecuzione su: CPU — il risultato della GPU era diverso da quello della CPU
        [failed] Esecuzione su: CPU — il modello non è riuscito sulla GPU
        [setting] Esecuzione su: CPU (elaborazione impostata su «Solo CPU»)
        [checking] Esecuzione su: CPU — verifica della GPU in corso
       *[cpu] Esecuzione su: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } su disco
       *[download] { $bytes } da scaricare
    } · { $memory } di memoria{ $gpu ->
        [yes] { " · richiede la GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Richiede una GPU, e questo computer non ne ha una utilizzabile.
       *[memory] Richiede { $need } di memoria, più dei { $have } che Specular consente per l’IA su questo computer.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = È in uso { $current }, non { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Torna al predefinito
        [anyway] Usa comunque
       *[use] Usa questa dimensione
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Scelta, ma non ancora scaricata: resta in uso { $current }.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] In uso
        [downloaded] Scaricata, non in uso
        [missing] Non scaricata
        [previous] In uso finché la nuova dimensione non avrà indicizzato la libreria
        [target] Download…
        [failed] Download non riuscito
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Visualizza licenza
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: passare a { $size }? La libreria verrà indicizzata di nuovo; fino ad allora resta in uso la dimensione attuale.
       *[other] { $feature }: passare a { $size }? Verranno scaricati { $download } e la libreria verrà indicizzata di nuovo; fino ad allora resta in uso la dimensione attuale.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: usare comunque { $size }? Potrebbe non funzionare su questo computer, o solo molto lentamente.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: { $size } ora è più adatto a questo computer. Passare? { $download ->
        [none] La libreria verrà indicizzata di nuovo.
       *[other] Verranno scaricati { $download } e la libreria verrà indicizzata di nuovo.
    }
ai-suggest-toast = IA locale: un’altra dimensione del modello è più adatta a questo computer (Impostazioni › IA locale).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Passaggio a { $size } · { $done } di { $total } immagini indicizzate

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Disponibile in una versione successiva
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Molte lingue
        [english] Solo inglese
        [cjk-latin] Cinese, giapponese e alfabeto latino
        [zh-latin] Cinese e alfabeto latino
        [arabic] Arabo
        [hebrew] Ebraico
        [korean] Coreano
        [cyrillic] Cirillico
        [greek] Greco
        [thai] Thailandese
        [devanagari] Devanagari
        [tamil] Tamil
        [telugu] Telugu
        [eslav] Russo, ucraino e bielorusso
        [latin] Alfabeto latino (molte lingue europee)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Più veloce, per schede grafiche più lente
        [closed-eyes] Rileva gli occhi chiusi
        [small-faces] Migliore con volti piccoli
        [no-translate] Non traduce in inglese
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Dati di addestramento: { $source }
        [research] Dati di addestramento: { $source } (licenza solo per la ricerca)
       *[unstated] { $named ->
            [no] Dati di addestramento: non indicati, termini non dichiarati
           *[yes] Dati di addestramento: { $source }, termini non dichiarati
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Scaricato da { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Raggruppa i volti in persone
ai-grouping-help = Disattivato finché non lo attivi per questa libreria. Disattivarlo interrompe il raggruppamento; i volti già raggruppati restano finché non li elimini.
ai-grouping-state = { $on ->
        [yes] Attivo in questa libreria
       *[no] Disattivato
    }
ai-grouping-button = { $on ->
        [yes] Disattiva
       *[no] Attiva
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Scarica prima { $feature }.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Rimuovere { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = I suoi file del modello vengono eliminati. Le tue foto e il testo già trovato restano.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Leggi il testo nelle foto di questa libreria
# machine-drafted, needs native review
ai-text-indexing-help = Disattivato finché non lo attivi per questa libreria. Poi text: nella ricerca della Libreria trova le foto in base alle parole che contengono.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Attivo in questa libreria
       *[no] Disattivato
    }

## Installazione da file

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Copiato un file di modello. Premi «Scarica» per installarlo.
        [many] Copiati { $count } file di modello. Premi «Scarica» per installarli.
       *[other] Copiati { $count } file di modello. Premi «Scarica» per installarli.
    }
ai-sideload-none = Nessun file di modello trovato.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } non è un modello noto a { -app-name }. Non è stato copiato.
ai-sideload-failed = Impossibile copiare { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Usare un modello non verificato?
ai-unverified-detail = { $file } non è nell’elenco dei modelli forniti con { -app-name }, quindi non può essere verificato. Verrà usato come { $model }.
ai-unverified-use = Usa modello non verificato

# machine-drafted, needs native review
ai-failed-internal = Qualcosa è andato storto in { -app-name }. Il log contiene i dettagli.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Passa
       *[keep] Mantieni dimensione attuale
    }
