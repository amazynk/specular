# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. Suomi.

keys-mode-drawing = Piirustukset
cheatsheet-drawing = Piirustukset
palette-drawing-layers = Näytä tai piilota tasopaneeli
palette-drawing-all-layers = Näytä kaikki tasot tai tasot tallennetulla tavalla
palette-drawing-paper = Vaihda tumma ja vaalea paperi
palette-drawing-lineweights = Näytä tai piilota viivanpaksuudet
disabled-not-drawing = Vain piirustuksille (DXF)
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = Malli
# machine-drafted, needs native review
cad-pen-name = Kynä { $n }
cad-page-name = Sivu { $n }
cad-layer-name = Taso { $n }
cad-map-name = Kartta
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = Kerrokset
cad-board-top-name = Yläpuoli
cad-board-bottom-name = Alapuoli
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = Kehys
cad-trace-name = Käyrä { $n }
cad-contours-positive-name = Positiiviset korkeuskäyrät
cad-contours-negative-name = Negatiiviset korkeuskäyrät
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 3D-malli
# The layers panel's title (and its name for screen readers).
drawing-layers-title = Tasot · { $shown }/{ $total } näkyvissä
# Toasts.
drawing-layers-all-toast = Kaikki tasot näkyvissä
drawing-layers-saved-toast = Tasot piirustukseen tallennetulla tavalla
drawing-paper-dark-toast = Tumma paperi
drawing-paper-light-toast = Vaalea paperi
drawing-lineweights-on = Viivanpaksuudet näkyvissä
drawing-lineweights-off = Viivanpaksuudet pois: ohuet viivat

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = Merkitse
palette-markup-help = Pilvet, teksti, nuolet, leimat, mitat ja kuvanastat, tallennetaan tiedoston viereen.
palette-compare-sheets = Vertaa piirustuksia
palette-compare-sheets-help = Näytä tämän piirustuksen toinen revisio päällekkäin kohdistettuna, muutokset rajattuina.
markup-title = Merkinnät
markup-list = Tämän sivun merkinnät
markup-count = { $count ->
    [0] Tällä sivulla ei ole merkintöjä
    [one] 1 merkintä tällä sivulla
   *[other] { $count } merkintää tällä sivulla
}
markup-empty = Valitse työkalu ja napsauta piirustusta.
markup-item = { $n }. { $kind }
markup-tool-select = Valitse
markup-tool-cloud = Pilvi
markup-tool-text = Teksti
markup-tool-arrow = Nuoli
markup-tool-stamp = Leima
markup-tool-length = Pituus
markup-tool-area = Pinta-ala
markup-tool-pin = Kuvanasta
markup-hint-select = Valitse kohde napsauttamalla. Siirrä vetämällä. Enter muokkaa sen tekstiä.
markup-hint-cloud = Vedä suorakulmio, tai napsauta kulmia ja paina Enter.
markup-hint-text = Napsauta tekstin kohtaa.
markup-hint-arrow = Vedä selitteestä pisteeseen, tai napsauta molempia.
markup-hint-stamp = Napsauta leiman kohtaa.
markup-hint-length = Vedä, tai napsauta molempia päitä.
markup-hint-area = Napsauta kulmia ja paina sitten Enter.
markup-hint-pin = Napsauta pistettä ja valitse sitten kuva.
markup-kind-cloud = Pilvi
markup-kind-text = Teksti
markup-kind-arrow = Nuoli
markup-kind-stamp = Leima
markup-kind-length = Pituus
markup-kind-area = Pinta-ala
markup-kind-pin = Kuva
markup-colour = Väri: { $colour }
markup-colour-red = Punainen
markup-colour-blue = Sininen
markup-colour-green = Vihreä
markup-colour-orange = Oranssi
markup-colour-magenta = Magenta
markup-colour-black = Musta
markup-width = Viiva: { $width }
markup-width-thin = Ohut
markup-width-medium = Keskivahva
markup-width-thick = Paksu
markup-stamp = Leima: { $words }
markup-stamp-approved = Hyväksytty
markup-stamp-rejected = Hylätty
markup-stamp-reviewed = Tarkastettu
markup-stamp-custom = Oma teksti
markup-undo = Kumoa
markup-redo = Tee uudelleen
markup-delete = Poista
markup-open-photo = Avaa kuva
markup-from-measurements = Lisää mittaukset
markup-from-measurements-done = { $count ->
    [one] 1 mittaus lisätty merkinnäksi
   *[other] { $count } mittausta lisätty merkinnöiksi
}
markup-export-pdf = Vie PDF…
markup-export-png = Vie PNG…
markup-done = Valmis
markup-ask-text = Teksti (Enter sijoittaa, Esc peruu)
markup-ask-label = Seliteteksti (valinnainen; Enter sijoittaa)
markup-ask-stamp = Leiman teksti (Enter sijoittaa)
markup-ask-edit = Uusi teksti (Enter tallentaa)
markup-added = Lisätty: { $kind }
markup-moved = Siirretty
markup-pin-hint = Napsauta nastaa uudelleen tai paina Enter avataksesi sen kuvan.
markup-deleted = Poistettu
markup-undone = Kumottu
markup-redone = Tehty uudelleen
markup-delete-ask = Poistetaanko { $name }?
markup-delete-detail = Se poistetaan merkintätiedostosta. Itse piirustusta ei muuteta.
markup-delete-ok = Poista
markup-photo-missing = { $name } ei ole enää olemassa.
markup-load-failed = Merkintöjä ei luettu: { $reason }
markup-save-failed = Merkintöjä ei tallennettu: { $reason }
markup-sidecar-too-large = merkintätiedosto on liian suuri
markup-pdf-unreadable = PDF-tiedostoa ei voitu lukea
markup-exported = Tallennettu { $name }
markup-export-failed = Ei viety: { $reason }
markup-export-same-file = valitse toinen nimi; alkuperäistä ei koskaan korvata
markup-export-encrypted = PDF on salattu. Vie sen sijaan PNG.
markup-export-damaged = PDF:n rakenne on vioittunut. Vie sen sijaan PNG.
sheets-title = Vertaa piirustuksia
sheets-other = Toinen piirustus…
sheets-other-named = Toinen: { $name }
sheets-page = Sivu { $page }/{ $count }
sheets-auto = Kohdista
sheets-pick = Valitse 2 pistettä
sheets-overlay = Päällekkäin
sheets-difference = Erotus
sheets-blend = Häivytys
sheets-blink = Vilkutus
sheets-slider = Toinen piirustus { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] Ei muutoksia
    [one] Pilvitä 1 muutos
   *[other] Pilvitä { $count } muutosta
}
sheets-done = Valmis
sheets-choose = Valitse toinen piirustus.
sheets-reading = Luetaan piirustusta…
sheets-aligning = Kohdistetaan…
sheets-aligned = Kohdistettu: mittakaava { $scale }, kierto { $angle }°, poikkeama enintään { $error } px. { $changes ->
    [0] Muutoksia ei löytynyt.
    [one] 1 muutos.
   *[other] { $changes } muutosta.
}
sheets-align-failed = Automaattinen kohdistus ei onnistunut. Valitse 2 pistettä.
sheets-pick-this = Piste { $n }: napsauta sitä tässä piirustuksessa.
sheets-pick-other = Piste { $n }: napsauta samaa pistettä toisessa piirustuksessa.
sheets-pick-same = Pisteet ovat samat. Valitse 2 pistettä uudelleen.
sheets-read-failed = Piirustusta ei luettu: { $reason }
sheets-failed = Vaihe epäonnistui. Yritä uudelleen tai valitse toinen piirustus.
sheets-legend-this = Vain tämä piirustus
sheets-legend-other = Vain toinen piirustus
sheets-legend-both = Molemmat
sheets-cloud-text = Muutettu
sheets-clouds-added = { $count ->
    [0] Pilviä ei lisätty
    [one] 1 pilvi lisätty merkintöihin
   *[other] { $count } pilveä lisätty merkintöihin
}
sheets-opened = Vertaa piirustuksia
sheets-closed = Vertailu suljettu
markup-ask-new-line = Shift+Enter: uusi rivi

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = Piirustukset
settings-drawings-shx-folders = SHX-fonttien kansiot
settings-drawings-shx-folders-help = Kansiot, joissa on SHX-fontteja DXF- ja DWG-tekstiä varten, erotettuina merkillä ;. Piirustuksen omasta kansiosta haetaan ensin.
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = Ei kansio: { $folder }
