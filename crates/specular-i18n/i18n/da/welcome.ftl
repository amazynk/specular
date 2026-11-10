# machine-drafted, needs native review
# Specular — Startskærmen (når Specular åbnes uden en fil, og når det sidste billede lukkes),
# dens kort »Prøv dette« og tipsene, der kun vises én gang. Dansk.

## Startskærm

welcome-title = { -app-name }
welcome-tagline = En hurtig, tro fremviser til fotos og til tekniske, videnskabelige og medicinske billeder.
welcome-open = Åbn en fil…
welcome-open-about = Et billede, et dokument eller en datafil.
welcome-browse = Gennemse en mappe…
welcome-browse-about = Vælg en mappe, og se den som galleri.
welcome-pictures = Dine billeder
welcome-pictures-about = Alle fotos i dit bibliotek, efter dato og sted.
welcome-pictures-setup-title = Opret dit bibliotek…
welcome-pictures-setup = Med udgangspunkt i mappen { $folder }.
welcome-pictures-ask = Vil du føje din mappe { $folder } til biblioteket?
welcome-pictures-add = Tilføj mappe
welcome-drop = Du kan også slippe filer eller mapper hvor som helst i dette vindue.
welcome-recent = Seneste mapper
welcome-recent-name = { $name }, i { $parent }
welcome-tips = Prøv dette
welcome-tips-hide = Skjul disse tips
welcome-tips-hidden = Tips skjult. »Vis velkomst« i kommandopaletten henter dem tilbage.

## Prøv dette: et navn og én linje hver. Tasten ved navnet er brugerens egen.

welcome-tip-gallery = Galleri
welcome-tip-gallery-about = Se en hel mappe på én gang.
welcome-tip-compare = Sammenlign
welcome-tip-compare-about = Vælg to til fire billeder i galleriet for at se dem side om side.
welcome-tip-cull = Sorteringstilstand
welcome-tip-cull-about = Vælg, afvis og bedøm en fotoserie med én tast for hver.
welcome-tip-slideshow = Diasshow
welcome-tip-slideshow-about = Vis en mappe i fuld skærm, ét billede ad gangen.
welcome-tip-palette = Kommandopalet
welcome-tip-palette-about = Find enhver kommando eller indstilling efter navn.
welcome-tip-studies = Undersøgelser
welcome-tip-studies-about = Patienter, undersøgelser og serier på en DICOM-disk eller i en DICOM-mappe.
welcome-tip-dicom-servers = DICOMweb-servere
welcome-tip-dicom-servers-about = Gennemse og åbn undersøgelser fra et PACS.
welcome-tip-dicom-tags = DICOM-tags
welcome-tip-dicom-tags-about = Alle et billedes tags, søgbare.
welcome-tip-window-level = Vindue og niveau
welcome-tip-window-level-about = Træk for at indstille kontrasten i et DICOM-billede.
welcome-tip-omero-servers = OMERO-servere
welcome-tip-omero-servers-about = Gennemse mikroskopiprojekter på OMERO og IDR.
welcome-tip-stretch = Stræk
welcome-tip-stretch-about = Frem svage detaljer i astronomiske og videnskabelige data.
welcome-tip-drawing-layers = Tegningslag
welcome-tip-drawing-layers-about = Vis og skjul lagene i en teknisk tegning.
welcome-tip-needs-image = Åbn først et billede, og tryk derefter på { $key }.
welcome-tip-needs-gallery = Vælg to til fire billeder i galleriet, og tryk derefter på { $key }.
welcome-tip-key = Genvej: { $key }

## Engangstips: hvert vises én gang, diskret, højst ét pr. start. Indstillinger › Generelt ›
## Vis tips slår dem alle fra.

hint-gallery = Tryk på { $key } for at se denne mappe som galleri.
hint-gallery-first = Vælg to til fire billeder, og tryk på { $compare } for at sammenligne dem eller på { $cull } for at sortere mappen.
hint-raw = En mappe med raw-fotos: tryk på { $key } for at sortere den (vælg, afvis og bedøm).
hint-pages =
    { $count ->
        [one] Denne fil har { $count } side. { $next } og { $prev } bladrer.
       *[other] Denne fil har { $count } sider. { $next } og { $prev } bladrer.
    }
hint-turn-off = Slå tips fra
hint-turned-off = Tips slået fra. Indstillinger › Generelt › Vis tips slår dem til igen.

## Kommandopalet

palette-show-welcome = Vis velkomst
palette-show-welcome-help = Startskærmen med Åbn, Gennemse, dine billeder og ting at prøve.
palette-close = Luk
palette-close-help = Luk billedet eller mappen, og gå tilbage til startskærmen. På startskærmen: afslut.

## Første start (under profilspørgsmålet)

profile-on-start = Når { -app-name } åbnes uden en fil:
profile-skip = Spring over
