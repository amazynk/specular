# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). German. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = Lokale KI
settings-ai-pack = Lokales KI-Paket
# machine-drafted, needs native review
settings-ai-pack-help = Optionale Funktionen, die nur auf diesem Computer laufen. Nichts, was sie finden, verlässt ihn. Wählen Sie für jede Funktion ein Modell; heruntergeladen wird erst, wenn Sie es auswählen.
# machine-drafted, needs native review
settings-ai-install-from-file = Aus Datei installieren …
settings-ai-install-from-file-help = Modelldateien installieren, die Sie auf diesen Computer kopiert haben, für Rechner ohne Internet. Jede Datei wird mit der Liste geprüft, die mit { -app-name } ausgeliefert wird.
settings-ai-install-from-folder = Aus Ordner installieren …
settings-ai-install-from-folder-help = Dasselbe für jede Modelldatei in einem Ordner.
settings-ai-search = Suche nach Beschreibung
settings-ai-search-help = Fotos finden, indem Sie sie beschreiben, etwa „Hund am Strand“, und ähnliche Aufnahmen besser gruppieren.
settings-ai-text = Text in Bildern
settings-ai-text-help = Den Text in Fotos und Scans lesen, damit die Suche mit text: sie findet.
settings-ai-culling = Hilfe beim Sichten
# machine-drafted, needs native review
settings-ai-culling-help = Geschlossene Augen, Unschärfe und schlechte Belichtung markieren, um die beste Aufnahme einer Serie zu finden.
settings-ai-people = Personen
# machine-drafted, needs native review
settings-ai-people-help = Gesichter finden und nach Person gruppieren, für die Suche mit people:. Die Gruppierung bleibt aus, bis Sie sie für eine Bibliothek einschalten. Gesichtsdaten lassen sich für eine Person oder für alle auf einmal löschen.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] In diesem Build sind keine Modelle verfügbar.
        [installed] Installiert
        [indexing] Installiert · { $done } von { $total } Bildern indiziert
        [downloading] Wird heruntergeladen… { $percent }
        [failed] Download fehlgeschlagen
       *[available] Nicht installiert
    }
ai-removing = Wird entfernt…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Herunterladen
        [cancel] Abbrechen
        [remove] Entfernen
        [forget] Vergessen
        [faces] Alle Gesichtsdaten löschen
       *[pack] Paket entfernen
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Alle Gesichtsdaten löschen? Alle gefundenen Gesichter und Personen werden gelöscht. Ihre Fotos bleiben erhalten.
       *[pack] Das lokale KI-Paket, seine Modelle und alles Gefundene entfernen? Ihre Fotos werden nicht verändert.
    }

## Gallery search
gallery-query-needs-ai = { $field }: braucht das lokale KI-Paket (Einstellungen › Lokale KI)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: das lokale KI-Paket kann das noch nicht durchsuchen

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Gesichter eingeblendet
# machine-drafted, needs native review
faces-hidden = Gesichter ausgeblendet
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Augen geschlossen
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Augen geschlossen
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Augen geschlossen: { $closed } von { $faces } Gesichtern
# machine-drafted, needs native review
cull-flag-blurred = Unscharf
# machine-drafted, needs native review
cull-flag-dark = Zu dunkel
# machine-drafted, needs native review
cull-flag-bright = Zu hell
# machine-drafted, needs native review
cull-flag-clipped = Lichter oder Schatten beschnitten
# machine-drafted, needs native review
# The People view.
people-title = Personen
# machine-drafted, needs native review
people-open = Personen
# machine-drafted, needs native review
people-done = Fertig
# machine-drafted, needs native review
people-back = Zurück
# machine-drafted, needs native review
people-show-photos = Fotos anzeigen
# machine-drafted, needs native review
people-name = Benennen…
# machine-drafted, needs native review
people-name-prompt = Diese Person benennen
# machine-drafted, needs native review
people-faces-open = Gesichter
# machine-drafted, needs native review
people-faces-of = Gesichter von { $name }
# machine-drafted, needs native review
people-merge = Zusammenführen
# machine-drafted, needs native review
people-merged = { $count ->
        [one] { $count } Person zusammengeführt
       *[other] { $count } Personen zusammengeführt
    }
# machine-drafted, needs native review
people-merge-ask = { $from } mit { $into } zusammenführen?
# machine-drafted, needs native review
people-merge-ask-many = { $count } Personen mit { $into } zusammenführen?
# machine-drafted, needs native review
people-merge-ask-detail = Die Gesichter gehören dann zu { $into }. Fotos auf dem Datenträger bleiben unverändert.
# machine-drafted, needs native review
people-not-this = Nicht diese Person
# machine-drafted, needs native review
people-face-removed = Gesicht von dieser Person entfernt. Das Foto bleibt erhalten.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Person #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } Gesicht
       *[other] { $count } Gesichter
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } Person
       *[other] { $count } Personen
    }
# machine-drafted, needs native review
people-face-a11y = Gesicht { $index }
# machine-drafted, needs native review
people-loading = Wird geladen…
# machine-drafted, needs native review
people-not-installed = Personen ist nicht installiert. Laden Sie es unter Einstellungen › Lokale KI herunter.
# machine-drafted, needs native review
people-grouping-off = Das Gruppieren von Gesichtern ist für diese Bibliothek aus. Schalten Sie „Gesichter zu Personen gruppieren“ unter Einstellungen › Lokale KI ein.
# machine-drafted, needs native review
people-none = Noch keine Personen gefunden.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Modell
settings-ai-search-size-help = SigLIP 2 B/16 findet Fotos am besten, in vielen Sprachen. B/32 indiziert etwa viermal schneller, für langsamere Grafikkarten.
settings-ai-search-size-recommended = Standard
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Modell
settings-ai-text-size-help = PP-OCRv6 small liest genauer, tiny ist schneller.
settings-ai-text-size-recommended = Standard
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Groß
# machine-drafted, needs native review
settings-ai-culling-size = Modell
settings-ai-culling-size-help = Das Modell, das geschlossene Augen erkennt.
settings-ai-culling-size-recommended = Standard
settings-ai-culling-size-small = Klein
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Groß
# machine-drafted, needs native review
settings-ai-people-size = Modell
settings-ai-people-size-help = SFace ist klein. AuraFace ist größer und mit Daten trainiert, die sein Herausgeber als für kommerzielle Nutzung lizenziert bezeichnet.
settings-ai-people-size-recommended = Standard
settings-ai-people-size-small = Klein
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Klein
        [large] Groß
       *[standard] Standard
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Standard ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Standard für Ihre { $gpu }: { $size }
ai-recommended = Standard für diesen Computer: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Für den Arbeitsspeicher dieses Computers passt keine Größe.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Läuft auf: GPU ({ $gpu })
        [gpu-unnamed] Läuft auf: GPU
        [no-gpu] Läuft auf: CPU (keine GPU gefunden)
        [differed] Läuft auf: CPU – das GPU-Ergebnis wich von dem der CPU ab
        [failed] Läuft auf: CPU – das Modell ist auf der GPU gescheitert
        [setting] Läuft auf: CPU (Verarbeitung ist auf „Nur CPU“ gestellt)
        [checking] Läuft auf: CPU – die GPU wird damit abgeglichen
       *[cpu] Läuft auf: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } auf dem Datenträger
       *[download] { $bytes } Download
    } · { $memory } Arbeitsspeicher{ $gpu ->
        [yes] { " · benötigt die GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Benötigt eine GPU, und dieser Computer hat keine, die er nutzen kann.
       *[memory] Benötigt { $need } Arbeitsspeicher, mehr als die { $have }, die Specular auf diesem Computer für KI zulässt.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = { $current } ist in Verwendung, nicht { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Zurück zum Standard
        [anyway] Trotzdem verwenden
       *[use] Diese Größe verwenden
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Ausgewählt, aber noch nicht heruntergeladen: { $current } bleibt in Verwendung.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] In Verwendung
        [downloaded] Heruntergeladen, nicht in Verwendung
        [missing] Nicht heruntergeladen
        [previous] In Verwendung, bis die neue Größe Ihre Bibliothek indiziert hat
        [target] Wird heruntergeladen…
        [failed] Download fehlgeschlagen
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Lizenz anzeigen
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: zu { $size } wechseln? Ihre Bibliothek wird neu indiziert; bis dahin bleibt die aktuelle Größe in Verwendung.
       *[other] { $feature }: zu { $size } wechseln? Dabei werden { $download } heruntergeladen und Ihre Bibliothek wird neu indiziert; bis dahin bleibt die aktuelle Größe in Verwendung.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: { $size } trotzdem verwenden? Es läuft auf diesem Computer möglicherweise nicht oder nur sehr langsam.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: { $size } passt jetzt besser zu diesem Computer. Wechseln? { $download ->
        [none] Ihre Bibliothek wird neu indiziert.
       *[other] Dabei werden { $download } heruntergeladen und Ihre Bibliothek wird neu indiziert.
    }
ai-suggest-toast = Lokale KI: Eine andere Modellgröße passt besser zu diesem Computer (Einstellungen › Lokale KI).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Wechsel zu { $size } · { $done } von { $total } Bildern indiziert

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Kommt in einer späteren Version
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Viele Sprachen
        [english] Nur Englisch
        [cjk-latin] Chinesisch, Japanisch und lateinische Schrift
        [zh-latin] Chinesisch und lateinische Schrift
        [arabic] Arabisch
        [hebrew] Hebräisch
        [korean] Koreanisch
        [cyrillic] Kyrillisch
        [greek] Griechisch
        [thai] Thai
        [devanagari] Devanagari
        [tamil] Tamil
        [telugu] Telugu
        [eslav] Russisch, Ukrainisch und Belarussisch
        [latin] Lateinische Schrift (viele europäische Sprachen)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Schneller, für langsamere Grafikkarten
        [closed-eyes] Erkennt geschlossene Augen
        [small-faces] Besser bei kleinen Gesichtern
        [no-translate] Kann nicht ins Englische übersetzen
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Trainingsdaten: { $source }
        [research] Trainingsdaten: { $source } (nur für die Forschung lizenziert)
       *[unstated] { $named ->
            [no] Trainingsdaten: nicht genannt, Bedingungen nicht angegeben
           *[yes] Trainingsdaten: { $source }, Bedingungen nicht angegeben
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Wird von { $host } heruntergeladen
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Gesichter zu Personen gruppieren
ai-grouping-help = Aus, bis Sie es für diese Bibliothek einschalten. Ausschalten beendet das Gruppieren; bereits gruppierte Gesichter bleiben, bis Sie sie löschen.
ai-grouping-state = { $on ->
        [yes] In dieser Bibliothek an
       *[no] Aus
    }
ai-grouping-button = { $on ->
        [yes] Ausschalten
       *[no] Einschalten
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Laden Sie zuerst { $feature } herunter.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = { $name } entfernen?
# machine-drafted, needs native review
ai-addon-remove-detail = Seine Modelldateien werden gelöscht. Ihre Fotos und der bereits gefundene Text bleiben erhalten.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Text in den Fotos dieser Bibliothek lesen
# machine-drafted, needs native review
ai-text-indexing-help = Aus, bis Sie es für diese Bibliothek einschalten. Dann findet text: in der Suche der Bibliothek Fotos anhand der Wörter darin.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] In dieser Bibliothek an
       *[no] Aus
    }

## Installieren aus Datei

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] Eine Modelldatei kopiert. Klicken Sie auf „Herunterladen“, um sie zu installieren.
       *[other] { $count } Modelldateien kopiert. Klicken Sie auf „Herunterladen“, um sie zu installieren.
    }
ai-sideload-none = Keine Modelldateien gefunden.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } ist kein Modell, das { -app-name } kennt. Es wurde nicht kopiert.
ai-sideload-failed = { $file } konnte nicht kopiert werden: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Ungeprüftes Modell verwenden?
ai-unverified-detail = { $file } steht nicht auf der Liste der mit { -app-name } ausgelieferten Modelle und kann daher nicht geprüft werden. Es wird als { $model } verwendet.
ai-unverified-use = Ungeprüftes Modell verwenden

# machine-drafted, needs native review
ai-failed-internal = In { -app-name } ist etwas schiefgelaufen. Das Protokoll enthält die Details.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Wechseln
       *[keep] Aktuelle Größe behalten
    }
