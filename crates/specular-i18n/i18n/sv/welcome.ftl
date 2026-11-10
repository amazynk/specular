# machine-drafted, needs native review
# Specular — Startskärmen (när Specular öppnas utan fil och när den sista bilden stängs),
# dess kort ”Att prova” och tipsen som bara visas en gång. Svenska.

## Startskärm

welcome-title = { -app-name }
welcome-tagline = En snabb och trogen bildvisare för foton och för tekniska, vetenskapliga och medicinska bilder.
welcome-open = Öppna en fil…
welcome-open-about = En bild, ett dokument eller en datafil.
welcome-browse = Bläddra i en mapp…
welcome-browse-about = Välj en mapp och se den som galleri.
welcome-pictures = Dina bilder
welcome-pictures-about = Alla foton i ditt bibliotek, efter datum och plats.
welcome-pictures-setup-title = Skapa ditt bibliotek…
welcome-pictures-setup = Med din mapp { $folder } som start.
welcome-pictures-ask = Lägga till din mapp { $folder } i biblioteket?
welcome-pictures-add = Lägg till mapp
welcome-drop = Du kan också släppa filer eller mappar var som helst i det här fönstret.
welcome-recent = Senaste mappar
welcome-recent-name = { $name }, i { $parent }
welcome-tips = Att prova
welcome-tips-hide = Dölj de här tipsen
welcome-tips-hidden = Tipsen är dolda. ”Visa välkomst” i kommandopaletten tar tillbaka dem.

## Att prova: ett namn och en rad vardera. Tangenten vid namnet är användarens egen.

welcome-tip-gallery = Galleri
welcome-tip-gallery-about = Se en hel mapp på en gång.
welcome-tip-compare = Jämför
welcome-tip-compare-about = Markera två till fyra bilder i galleriet för att se dem sida vid sida.
welcome-tip-cull = Gallringsläge
welcome-tip-cull-about = Välj, förkasta och betygsätt en fotografering med en tangent var.
welcome-tip-slideshow = Bildspel
welcome-tip-slideshow-about = Visa en mapp i helskärm, en bild i taget.
welcome-tip-palette = Kommandopalett
welcome-tip-palette-about = Hitta valfritt kommando eller inställning efter namn.
welcome-tip-studies = Undersökningar
welcome-tip-studies-about = Patienter, undersökningar och serier på en DICOM-skiva eller i en DICOM-mapp.
welcome-tip-dicom-servers = DICOMweb-servrar
welcome-tip-dicom-servers-about = Bläddra bland och öppna undersökningar från ett PACS.
welcome-tip-dicom-tags = DICOM-taggar
welcome-tip-dicom-tags-about = Alla taggar i en bild, sökbara.
welcome-tip-window-level = Fönster och nivå
welcome-tip-window-level-about = Dra för att ställa in kontrasten i en DICOM-bild.
welcome-tip-omero-servers = OMERO-servrar
welcome-tip-omero-servers-about = Bläddra bland mikroskopiprojekt på OMERO och IDR.
welcome-tip-stretch = Sträckning
welcome-tip-stretch-about = Lyft fram svaga detaljer i astronomiska och vetenskapliga data.
welcome-tip-drawing-layers = Ritningslager
welcome-tip-drawing-layers-about = Visa och dölj lagren i en teknisk ritning.
welcome-tip-needs-image = Öppna först en bild och tryck sedan på { $key }.
welcome-tip-needs-gallery = Markera två till fyra bilder i galleriet och tryck sedan på { $key }.
welcome-tip-key = Kortkommando: { $key }

## Engångstips: vart och ett en gång, diskret, högst ett per start. Inställningar › Allmänt ›
## Visa tips stänger av alla.

hint-gallery = Tryck på { $key } för att se den här mappen som galleri.
hint-gallery-first = Markera två till fyra bilder och tryck på { $compare } för att jämföra dem eller på { $cull } för att gallra mappen.
hint-raw = En mapp med raw-foton: tryck på { $key } för att gallra den (välj, förkasta och betygsätt).
hint-pages =
    { $count ->
        [one] Filen har { $count } sida. { $next } och { $prev } bläddrar.
       *[other] Filen har { $count } sidor. { $next } och { $prev } bläddrar.
    }
hint-turn-off = Stäng av tips
hint-turned-off = Tips avstängda. Inställningar › Allmänt › Visa tips sätter på dem igen.

## Kommandopalett

palette-show-welcome = Visa välkomst
palette-show-welcome-help = Startskärmen med Öppna, Bläddra, dina bilder och saker att prova.
palette-close = Stäng
palette-close-help = Stäng bilden eller mappen och gå tillbaka till startskärmen. På startskärmen: avsluta.

## Första start (under profilfrågan)

profile-on-start = När { -app-name } öppnas utan fil:
profile-skip = Hoppa över
