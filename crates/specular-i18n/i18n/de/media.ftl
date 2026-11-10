# Animationswiedergabe und Lesemodus (Archive, Comics).

disabled-not-animated = Dieses Bild ist nicht animiert
disabled-in-archive = Seiten in einem Archiv können nicht in den Papierkorb verschoben werden

## Befehlspalette und Tastenübersicht

palette-play-pause = Animation abspielen oder anhalten
palette-next-frame = Nächstes Einzelbild
palette-prev-frame = Vorheriges Einzelbild
palette-faster = Schneller abspielen
palette-slower = Langsamer abspielen
palette-normal-speed = Normale Wiedergabegeschwindigkeit
palette-loop-forever = Animation endlos wiederholen
palette-right-to-left = Seiten von rechts nach links
# machine-drafted, needs native review
palette-contents = Inhaltsverzeichnis
palette-spreads = Doppelseiten

## Animationssteuerung

transport-region = Animationssteuerung
transport-play = Abspielen
transport-pause = Anhalten
transport-prev-frame = Vorheriges Einzelbild
transport-next-frame = Nächstes Einzelbild
transport-slower = Langsamer
transport-faster = Schneller
transport-frame = Bild { $index } / { $count }
transport-frame-unknown = Bild { $index }
transport-speed = { $speed }×
transport-speed-name = Wiedergabegeschwindigkeit { $speed }×
# machine-drafted, needs native review
motion-badge = Bewegt
motion-badge-live = Live
motion-play = Bewegungsclip abspielen
motion-play-live = Live Photo abspielen
motion-stop = Zurück zum Standbild
toast-motion-failed = Bewegungsclip kann nicht abgespielt werden: { $reason }

## Meldungen

toast-speed = Wiedergabegeschwindigkeit { $speed }×
toast-loop-forever-on = Endlos wiederholen
toast-loop-forever-off = Wiederholen wie in der Datei angegeben
toast-right-to-left-on = Seiten von rechts nach links
toast-right-to-left-off = Seiten von links nach rechts
toast-spreads-on = Doppelseiten
toast-spreads-off = Einzelseiten

## Archive

title-archive-page = { $page } – { $archive }
readout-position-spread = { $first }–{ $last } / { $count }
archive-needs-sandbox = { $format }-Archive kann nur der abgeschottete Decoder-Prozess öffnen, der in diesem Build noch fehlt.
archive-empty = Dieses Archiv enthält keine Bilder.
archive-failed = Das Archiv konnte nicht geöffnet werden: { $error }
archive-encrypted = Dieses Archiv ist verschlüsselt. Öffnen Sie es erneut, um das Passwort einzugeben.
archive-wrong-password = Das Passwort für dieses Archiv ist falsch.
password-title = Passwort erforderlich
password-archive = „{ $archive }“ ist verschlüsselt.
password-label = Passwort
password-show = Passwort anzeigen
password-wrong = Falsches Passwort. Versuchen Sie es erneut.
password-open = Öffnen
password-cancel = Abbrechen

## Einstellungen

settings-viewing-animation-autoplay = Animationen automatisch abspielen
settings-viewing-animation-autoplay-help = Animierte Bilder (GIF, APNG, WebP …) beim Öffnen sofort abspielen.
settings-viewing-animation-loop = Animationen wiederholen
settings-viewing-animation-loop-help = Wie oft Animationen wiederholt werden.
settings-viewing-animation-loop-file = Wie in der Datei angegeben
settings-viewing-animation-loop-forever = Endlos
settings-viewing-animation-loop-once = Einmal abspielen
settings-viewing-animation-short-delays = Sehr kurze Bildverzögerungen
settings-viewing-animation-short-delays-help = Bilder, die 10 ms oder kürzer stehen sollen: wie Webbrowser auf 100 ms verlangsamen oder wie gespeichert abspielen.
settings-viewing-animation-short-delays-browser = Wie Webbrowser (100 ms)
settings-viewing-animation-short-delays-as-stored = Wie gespeichert
settings-browsing-reading-order = Seitenreihenfolge in Archiven
settings-browsing-reading-order-help = Comic-Archive werden von rechts nach links gelesen, wenn ihre ComicInfo.xml das angibt (Manga), sofern hier nichts anderes eingestellt ist.
settings-browsing-reading-order-auto = Aus dem Archiv
settings-browsing-reading-order-left-to-right = Von links nach rechts
settings-browsing-reading-order-right-to-left = Von rechts nach links
settings-browsing-page-layout = Seiten in Archiven
settings-browsing-page-layout-help = Eine Seite oder zwei nebeneinander zeigen; das Titelbild steht immer allein.
settings-browsing-page-layout-auto = Aus dem Archiv
settings-browsing-page-layout-single = Einzelseite
settings-browsing-page-layout-spreads = Doppelseiten

## Pages of multi-image files (ICNS sizes, texture layers, faces and slices, TIFF pages) (machine-drafted, needs native review)
palette-next-page = Nächste Seite
palette-prev-page = Vorherige Seite
palette-first-page = Erste Seite
palette-last-page = Letzte Seite
disabled-one-page = Das Bild hat nur eine Seite
page-indicator = Seite { $index } von { $count }
page-indicator-labelled = { $page }, { $label }
page-label-icon = { $width }×{ $height }
page-label-icon-density = { $width }×{ $height } @{ $density }x
page-label-layer = Ebene { $index }
page-label-face = Fläche { $face }
page-label-sphere = Sphere-Map
page-label-slice = Schicht { $index }
page-label-join = { $a } · { $b }
page-label-slide = Objektträger
page-label-slide-label = Etikett
page-label-slide-macro = Übersicht
page-label-slide-thumbnail = Miniatur
# machine-drafted, needs native review
page-label-thermal = Wärmebild
page-label-visible = Sichtbares Foto
# machine-drafted, needs native review
page-label-left-view = Linke Ansicht
# machine-drafted, needs native review
page-label-right-view = Rechte Ansicht
# machine-drafted, needs native review
# An object of a CERN ROOT file in the page picker: its path in the file and its class, e.g.
# "run1/hits (TH2D)".
page-label-object = { $name } ({ $class })
# machine-drafted, needs native review
# A view of one step of a circuit board job ("panel · Top").
page-label-board-step = { $step } · { $view }
# machine-drafted, needs native review
# A focal plane and a time point of a remote microscopy image (OMERO), counted from 1.
page-label-z = Z { $index } von { $count }
page-label-t = T { $index } von { $count }
# machine-drafted, needs native review
# Views of a remote-sensing product (an ENVI cube, a Sentinel-2 product) in the page picker.
page-label-product-composite = Standardbänder
page-label-product-bands = Alle Bänder
page-label-product-true-colour = Echtfarben
page-label-product-false-colour = Falschfarben (nahes Infrarot)
page-label-product-swir = Kurzwelliges Infrarot
page-label-product-agriculture = Landwirtschaft
page-label-product-scene-class = Szenenklassifikation
page-label-product-cloud = Wolkenwahrscheinlichkeit
# A product view at a pixel size, e.g. "True colour, 10 m".
page-label-product-at = { $view }, { $metres } m
title-file-page = { $file } – { $page }
info-page-group = Seite
info-page-position = Position
info-page-contents = Inhalt

# machine-drafted, needs native review
# OpenColorIO (Settings › Colour): why scene-linear images are shown without it.
ocio-failed = OCIO wird nicht verwendet: { $reason }
ocio-builtin-config = $OCIO verweist auf eine integrierte Konfiguration (ocio://); speichern Sie sie als Datei und wählen Sie diese Datei aus.

## Page labels for icons and collections (machine-drafted, needs native review)
page-label-size = { $width }×{ $height }
page-label-size-bits = { $width }×{ $height } · { $bits } Bit

## Page labels for Photoshop layers (machine-drafted, needs native review)
page-label-merged = Zusammengefügtes Bild
page-label-from-layers = Aus Ebenen zusammengesetzt
page-label-layer-hidden = { $name } (ausgeblendet)
page-label-layer-unnamed = Unbenannte Ebene { $index }
