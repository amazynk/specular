# machine-drafted, needs native review
# Specular — La schermata iniziale (quando Specular si apre senza un file e quando si chiude
# l’ultima immagine), le sue schede «Da provare» e i suggerimenti mostrati una sola volta. Italiano.

## Schermata iniziale

welcome-title = { -app-name }
welcome-tagline = Un visualizzatore veloce e fedele per foto e per immagini tecniche, scientifiche e mediche.
welcome-open = Apri un file…
welcome-open-about = Un’immagine, un documento o un file di dati.
welcome-browse = Sfoglia una cartella…
welcome-browse-about = Scegli una cartella e vedila come galleria.
welcome-pictures = Le tue immagini
welcome-pictures-about = Tutte le foto della tua libreria, per data e luogo.
welcome-pictures-setup-title = Configura la libreria…
welcome-pictures-setup = A partire dalla cartella { $folder }.
welcome-pictures-ask = Aggiungere la cartella { $folder } alla libreria?
welcome-pictures-add = Aggiungi cartella
welcome-drop = Puoi anche trascinare file o cartelle in qualsiasi punto di questa finestra.
welcome-recent = Cartelle recenti
welcome-recent-name = { $name }, in { $parent }
welcome-tips = Da provare
welcome-tips-hide = Nascondi questi suggerimenti
welcome-tips-hidden = Suggerimenti nascosti. «Mostra benvenuto» nella tavolozza dei comandi li riporta.

## Da provare: un nome e una riga ciascuno. Il tasto accanto al nome è quello dell’utente.

welcome-tip-gallery = Galleria
welcome-tip-gallery-about = Vedi un’intera cartella in un colpo d’occhio.
welcome-tip-compare = Confronta
welcome-tip-compare-about = Seleziona da due a quattro immagini nella galleria per vederle affiancate.
welcome-tip-cull = Modalità selezione
welcome-tip-cull-about = Scegli, scarta e valuta un servizio con un tasto ciascuno.
welcome-tip-slideshow = Presentazione
welcome-tip-slideshow-about = Mostra una cartella a schermo intero, un’immagine dopo l’altra.
welcome-tip-palette = Tavolozza dei comandi
welcome-tip-palette-about = Trova qualsiasi comando o impostazione per nome.
welcome-tip-studies = Studi
welcome-tip-studies-about = I pazienti, gli studi e le serie di un disco o di una cartella DICOM.
welcome-tip-dicom-servers = Server DICOMweb
welcome-tip-dicom-servers-about = Sfoglia e apri studi da un PACS.
welcome-tip-dicom-tags = Tag DICOM
welcome-tip-dicom-tags-about = Tutti i tag di un’immagine, con ricerca.
welcome-tip-window-level = Finestra e livello
welcome-tip-window-level-about = Trascina per regolare il contrasto di un’immagine DICOM.
welcome-tip-omero-servers = Server OMERO
welcome-tip-omero-servers-about = Sfoglia progetti di microscopia su OMERO e IDR.
welcome-tip-stretch = Stretch
welcome-tip-stretch-about = Fai emergere i dettagli deboli nei dati astronomici e scientifici.
welcome-tip-drawing-layers = Layer del disegno
welcome-tip-drawing-layers-about = Mostra e nascondi i layer di un disegno tecnico.
welcome-tip-needs-image = Apri prima un’immagine, poi premi { $key }.
welcome-tip-needs-gallery = Nella galleria, seleziona da due a quattro immagini, poi premi { $key }.
welcome-tip-key = Scorciatoia: { $key }

## Suggerimenti unici: una volta ciascuno, con discrezione, al massimo uno per avvio.
## Impostazioni › Generali › Mostra suggerimenti li disattiva tutti.

hint-gallery = Premi { $key } per vedere questa cartella come galleria.
hint-gallery-first = Seleziona da due a quattro immagini e premi { $compare } per confrontarle, o { $cull } per selezionare la cartella.
hint-raw = Una cartella di foto RAW: premi { $key } per selezionarla (scegli, scarta e valuta).
hint-pages =
    { $count ->
        [one] Questo file ha { $count } pagina. { $next } e { $prev } la sfogliano.
        [many] Questo file ha { $count } pagine. { $next } e { $prev } le sfogliano.
       *[other] Questo file ha { $count } pagine. { $next } e { $prev } le sfogliano.
    }
hint-turn-off = Disattiva suggerimenti
hint-turned-off = Suggerimenti disattivati. Impostazioni › Generali › Mostra suggerimenti li riattiva.

## Tavolozza dei comandi

palette-show-welcome = Mostra benvenuto
palette-show-welcome-help = La schermata iniziale, con Apri, Sfoglia, le tue immagini e cose da provare.
palette-close = Chiudi
palette-close-help = Chiudi l’immagine o la cartella e torna alla schermata iniziale. Nella schermata iniziale, esci.

## Primo avvio (sotto la domanda sul profilo)

profile-on-start = Quando { -app-name } si apre senza un file:
profile-skip = Salta
