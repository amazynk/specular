# machine-drafted, needs native review
# Specular — Het startscherm (als Specular zonder bestand opent en als de laatste afbeelding
# wordt gesloten), de kaarten ‘Om te proberen’ en de eenmalige tips. Nederlands.

## Startscherm

welcome-title = { -app-name }
welcome-tagline = Een snelle, getrouwe viewer voor foto’s en voor technische, wetenschappelijke en medische beelden.
welcome-open = Bestand openen…
welcome-open-about = Een afbeelding, een document of een gegevensbestand.
welcome-browse = Map bekijken…
welcome-browse-about = Kies een map en bekijk die als galerij.
welcome-pictures = Je afbeeldingen
welcome-pictures-about = Alle foto’s in je bibliotheek, op datum en plaats.
welcome-pictures-setup-title = Bibliotheek instellen…
welcome-pictures-setup = Te beginnen met je map { $folder }.
welcome-pictures-ask = Je map { $folder } aan de bibliotheek toevoegen?
welcome-pictures-add = Map toevoegen
welcome-drop = Je kunt ook bestanden of mappen ergens in dit venster neerzetten.
welcome-recent = Recente mappen
welcome-recent-name = { $name }, in { $parent }
welcome-tips = Om te proberen
welcome-tips-hide = Deze tips verbergen
welcome-tips-hidden = Tips verborgen. ‘Welkom tonen’ in het opdrachtenpalet haalt ze terug.

## Om te proberen: elk een naam en één regel. De toets naast de naam is die van de gebruiker.

welcome-tip-gallery = Galerij
welcome-tip-gallery-about = Een hele map in één oogopslag.
welcome-tip-compare = Vergelijken
welcome-tip-compare-about = Selecteer twee tot vier afbeeldingen in de galerij om ze naast elkaar te zien.
welcome-tip-cull = Selectiemodus
welcome-tip-cull-about = Een fotoshoot kiezen, afwijzen en beoordelen, met één toets per actie.
welcome-tip-slideshow = Diavoorstelling
welcome-tip-slideshow-about = Een map schermvullend tonen, de ene afbeelding na de andere.
welcome-tip-palette = Opdrachtenpalet
welcome-tip-palette-about = Elke opdracht of instelling op naam vinden.
welcome-tip-studies = Onderzoeken
welcome-tip-studies-about = De patiënten, onderzoeken en series van een DICOM-schijf of -map.
welcome-tip-dicom-servers = DICOMweb-servers
welcome-tip-dicom-servers-about = Onderzoeken uit een PACS bekijken en openen.
welcome-tip-dicom-tags = DICOM-tags
welcome-tip-dicom-tags-about = Alle tags van een afbeelding, doorzoekbaar.
welcome-tip-window-level = Venster en niveau
welcome-tip-window-level-about = Sleep om het contrast van een DICOM-afbeelding in te stellen.
welcome-tip-omero-servers = OMERO-servers
welcome-tip-omero-servers-about = Microscopieprojecten op OMERO en IDR bekijken.
welcome-tip-stretch = Stretch
welcome-tip-stretch-about = Zwakke details in astronomische en wetenschappelijke gegevens zichtbaar maken.
welcome-tip-drawing-layers = Tekeninglagen
welcome-tip-drawing-layers-about = De lagen van een technische tekening tonen en verbergen.
welcome-tip-needs-image = Open eerst een afbeelding en druk dan op { $key }.
welcome-tip-needs-gallery = Selecteer in de galerij twee tot vier afbeeldingen en druk dan op { $key }.
welcome-tip-key = Sneltoets: { $key }

## Eenmalige tips: elk één keer, onopvallend, hooguit één per start. Instellingen › Algemeen ›
## Tips tonen zet ze allemaal uit.

hint-gallery = Druk op { $key } om deze map als galerij te zien.
hint-gallery-first = Selecteer twee tot vier afbeeldingen en druk op { $compare } om ze te vergelijken, of op { $cull } om de map te selecteren.
hint-raw = Een map met raw-foto’s: druk op { $key } om te selecteren (kiezen, afwijzen en beoordelen).
hint-pages =
    { $count ->
        [one] Dit bestand heeft { $count } pagina. { $next } en { $prev } bladeren.
       *[other] Dit bestand heeft { $count } pagina’s. { $next } en { $prev } bladeren.
    }
hint-turn-off = Tips uitzetten
hint-turned-off = Tips uitgezet. Instellingen › Algemeen › Tips tonen zet ze weer aan.

## Opdrachtenpalet

palette-show-welcome = Welkom tonen
palette-show-welcome-help = Het startscherm, met Openen, Bekijken, je afbeeldingen en dingen om te proberen.
palette-close = Sluiten
palette-close-help = De afbeelding of map sluiten en teruggaan naar het startscherm. Op het startscherm: afsluiten.

## Eerste start (onder de profielvraag)

profile-on-start = Als { -app-name } zonder bestand opent:
profile-skip = Overslaan
