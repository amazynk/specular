# machine-drafted, needs native review
# Operazioni sui file: rinomina, rinomina multipla, copia e spostamento in una cartella,
# rotazione salvata nel file, annulla e ripeti, sfondo, condivisione, invio per e-mail,
# esportazione e importazione delle impostazioni.

## Palette, scheda rapida e menu

palette-undo = Annulla
palette-undo-help = Annulla l’ultima modifica ai file: spostamento nel cestino, rinomina, copia o spostamento, rotazione.
palette-redo = Ripeti
palette-redo-help = Rifai ciò che l’ultimo annullamento ha tolto.
palette-rename = Rinomina…
palette-rename-help = Dai un nuovo nome al file; i file sidecar e la coppia RAW+JPEG lo seguono.
palette-batch-rename = Rinomina file…
palette-batch-rename-help = Rinomina i file selezionati, o tutta la cartella, con uno schema e un’anteprima.
palette-copy-to-folder = Copia nella cartella…
palette-copy-to-folder-help = Copia il file o la selezione in un’altra cartella.
palette-move-to-folder = Sposta nella cartella…
palette-move-to-folder-help = Sposta il file o la selezione in un’altra cartella.
palette-rotate-file-left = Ruota il file a sinistra
palette-rotate-file-left-help = Ruota il file stesso di un quarto a sinistra, senza perdita se il formato lo consente.
palette-rotate-file-right = Ruota il file a destra
palette-rotate-file-right-help = Ruota il file stesso di un quarto a destra, senza perdita se il formato lo consente.
palette-set-wallpaper-help = Usa l’immagine come sfondo della scrivania.
palette-share = Condividi…
palette-share-help = Invia il file a un’altra app con il menu di condivisione del sistema.
palette-send-by-email = Invia per e-mail…
palette-send-by-email-help = Apri una nuova e-mail nell’app di posta con il file allegato.
# machine-drafted, needs native review
palette-share-with-zynk = Condividi con Zynk…
palette-share-with-zynk-help = Crea un link web al file nell’app Zynk.
palette-send-with-zynk = Invia con Zynk…
palette-send-with-zynk-help = Invia il file a un contatto o a uno dei tuoi dispositivi con Zynk.
palette-export-settings = Esporta impostazioni…
palette-export-settings-help = Salva in un file tutte le impostazioni e le scorciatoie da tastiera.
palette-import-settings = Importa impostazioni…
palette-import-settings-help = Leggi le impostazioni da un file, dopo aver mostrato cosa cambierebbe.

disabled-in-archive-file = Questa pagina è dentro un archivio e non ha un file proprio
disabled-not-rotatable = Questo tipo di file non può registrare una rotazione
disabled-not-on-this-desktop = Il tuo desktop non lo offre

## Impostazioni

settings-browsing-rotate-trim-edges = Rifila i bordi JPEG per ruotare senza perdita
settings-browsing-rotate-trim-edges-help = Un JPEG le cui dimensioni non sono multiple di 8 o 16 pixel si può ruotare senza perdita solo tagliando gli ultimi pixel. Senza questa opzione cambia invece il tag di orientamento, e ogni volta ti viene proposta la rifilatura.
settings-browsing-rotate-keep-modified = Mantieni la data di modifica durante la rotazione
settings-browsing-rotate-keep-modified-help = Un file ruotato mantiene la data dell’ultima modifica, così resta al suo posto nell’ordinamento per data.
settings-browsing-recent-folders = Cartelle recenti
settings-browsing-recent-folders-help = Le ultime cartelle usate per copiare e spostare.

## Finestre

fileop-cancel = Annulla
rename-title = Rinomina
rename-field-label = Nome
rename-button = Rinomina
rename-sidecars-follow = Rinominati anche: { $files }
rename-extension-changed = Cambiare l’estensione non converte il file; altre app potrebbero non aprirlo.
rename-problem-empty = Scrivi un nome.
rename-problem-invalid-char = I nomi non possono contenere «{ $char }».
rename-problem-reserved = «{ $name }» è un nome che Windows riserva ai dispositivi.
rename-problem-too-long = Il nome è troppo lungo; può avere al massimo 255 byte.
rename-problem-dot = «.» e «..» non si possono usare come nomi.
rename-problem-trailing = I nomi non possono finire con uno spazio o un punto.
rename-problem-exists = Un altro file ha già questo nome.
rename-problem-read-only = Questa cartella non si può modificare.
rename-problem-duplicate = Due file avrebbero questo nome.

batch-rename-title =
    { $count ->
        [one] Rinomina { $count } file
       *[other] Rinomina { $count } file
    }
batch-rename-field-label = Schema
batch-rename-tokens = Usa { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } e { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } file sarà rinominato
       *[other] { $count } file saranno rinominati
    }{ $problems ->
        [0] .
        [one] ; { $problems } non può esserlo e resta com’è.
       *[other] ; { $problems } non possono esserlo e restano come sono.
    }
batch-rename-reading = Lettura di date e fotocamere…
batch-rename-preview = Anteprima
batch-rename-unchanged = mantiene il nome
batch-rename-more =
    { $count ->
        [one] Ancora { $count } sotto
       *[other] Altri { $count } sotto
    }

transfer-copy-title-one = Copia «{ $file }» in una cartella
transfer-copy-title =
    { $count ->
        [one] Copia { $count } file in una cartella
       *[other] Copia { $count } file in una cartella
    }
transfer-move-title-one = Sposta «{ $file }» in una cartella
transfer-move-title =
    { $count ->
        [one] Sposta { $count } file in una cartella
       *[other] Sposta { $count } file in una cartella
    }
transfer-recent = Cartelle recenti
transfer-folder = Cartella
transfer-choose-folder = Scegli una cartella…
transfer-copy-dialog = Copia nella cartella
transfer-move-dialog = Sposta nella cartella
transfer-sidecars =
    { $count ->
        [one] Viene incluso { $count } file sidecar.
       *[other] Vengono inclusi { $count } file sidecar.
    }
transfer-taken =
    { $count ->
        [one] { $count } nome è già usato in { $folder }.
       *[other] { $count } nomi sono già usati in { $folder }.
    }
transfer-when-taken = Se un nome è già usato
transfer-skip = Salta
transfer-keep-both = Mantieni entrambi
transfer-replace-option = Sostituisci
transfer-copy = Copia
transfer-move = Sposta
transfer-replace-title =
    { $count ->
        [one] Sostituire { $count } file in { $folder }?
       *[other] Sostituire { $count } file in { $folder }?
    }
transfer-replace-note = I file presenti finiscono nel cestino. Annulla li riporta indietro.
transfer-replace = Sostituisci
transfer-back = Indietro
transfer-stop = Interrompi

settings-file-import-title = Importa impostazioni
settings-file-import-dialog = Importa impostazioni
settings-file-export-dialog = Esporta impostazioni
settings-file-import-changes = Modifiche
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = nessun tasto
settings-file-import-unknown = Non è un’impostazione di questa versione; ignorata.
settings-file-import-keymap = Scorciatoie da tastiera
settings-file-import-nothing = Non cambierebbe nulla: queste impostazioni sono già attive.
settings-file-import-summary =
    { $count ->
        [one] { $count } modifica. L’importazione si può annullare in Impostazioni.
       *[other] { $count } modifiche. L’importazione si può annullare in Impostazioni.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } voce sembra una password o una chiave e non viene mai importata.
       *[other] { $count } voci sembrano password o chiavi e non vengono mai importate.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } comando utente non viene importato: i comandi avviano programmi. Se ti fidi, copialo tu nel file delle impostazioni.
       *[other] { $count } comandi utente non vengono importati: i comandi avviano programmi. Se ti fidi, copiali tu nel file delle impostazioni.
    }
settings-file-import-button = Importa

## Risultati

fileop-renamed = Rinominato in { $file }
fileop-renamed-count =
    { $count ->
        [one] { $count } file rinominato
       *[other] { $count } file rinominati
    }
fileop-copied =
    { $count ->
        [one] { $count } file copiato in { $folder }
       *[other] { $count } file copiati in { $folder }
    }
fileop-moved =
    { $count ->
        [one] { $count } file spostato in { $folder }
       *[other] { $count } file spostati in { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } saltato: il nome è già usato lì.
       *[other] { $count } saltati: i nomi sono già usati lì.
    }
fileop-stopped = Interrotto; ciò che è stato fatto resta.
fileop-failed-file =
    { $count ->
        [one] Impossibile copiare o spostare { $file }: { $reason }
       *[other] Impossibile copiare o spostare { $count } file; il primo, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Copia di { $count } file… { PERCENT($percent) }
       *[other] Copia di { $count } file… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Spostamento di { $count } file… { PERCENT($percent) }
       *[other] Spostamento di { $count } file… { PERCENT($percent) }
    }
fileop-done-status = Fatto
fileop-busy = L’ultima modifica ai file è ancora in corso; riprova tra un attimo.
fileop-nothing-to-redo = Niente da ripetere
fileop-undone = Annullato: { $what }
fileop-redone = Ripetuto: { $what }
fileop-kind-rename = rinomina
fileop-kind-batch-rename = rinomina di più file
fileop-kind-move = spostamento nella cartella
fileop-kind-copy = copia nella cartella
fileop-kind-rotate = rotazione
rename-failed = Impossibile rinominare: { $reason }

undo-problem-changed = Impossibile annullare: { $file } è stato modificato nel frattempo e annullando si perderebbe quella modifica.
undo-problem-missing = Impossibile annullare: { $file } non c’è più.
undo-problem-taken = Impossibile annullare: ora un altro file si chiama { $file }.
undo-problem-io = Impossibile annullare la modifica a { $file }: { $reason }

rotate-done-lossless = { $file } ruotato senza perdita.
rotate-done-lossless-trimmed = { $file } ruotato senza perdita; il bordo è stato rifilato a { $width } × { $height }.
rotate-done-tag = { $file } ruotato cambiando il tag di orientamento.
rotate-done-sidecar = Rotazione di { $file } salvata in { $sidecar }; il file RAW non viene mai modificato.
rotate-tag-unreadable = I dati dell’immagine non si leggono abbastanza da ruotarla.
rotate-tag-multi-picture = Contiene più di un’immagine, e ruotare la prima rovinerebbe le altre.
rotate-tag-trailer = Dopo l’immagine ci sono dati che la rotazione sposterebbe.
rotate-tag-not-aligned = Ruotare i pixel senza perdita la rifilerebbe a { $width } × { $height }.
rotate-trim-offer = Rifila e ruota
rotate-problem-unsupported = { $file } è un tipo di file che non può registrare una rotazione.
rotate-problem-too-large = { $file } è troppo grande da ruotare.
rotate-problem-damaged = { $file } è danneggiato dove è salvato l’orientamento: { $reason }
rotate-problem-io = Impossibile ruotare { $file }: { $reason }

share-failed = Impossibile condividere: { $reason }
email-failed = Impossibile avviare l’app di posta: { $reason }
# machine-drafted, needs native review
zynk-opening = Apertura di Zynk…
zynk-failed = Impossibile contattare Zynk: { $reason }
zynk-get-title-send = «Invia con Zynk» richiede l’app Zynk
zynk-get-title-share = «Condividi con Zynk» richiede l’app Zynk
zynk-get-about = Zynk è un’app gratuita per Mac, Windows, Linux, iPhone e Android. Invia foto, video e intere cartelle direttamente agli altri tuoi dispositivi e ad altre persone, di qualsiasi dimensione. I trasferimenti sono crittografati end-to-end e riprendono da dove si erano fermati se la connessione cade. Per un destinatario senza Zynk, crea un link web da cui scaricare i file nel browser.
zynk-get-macos = Scaricala dal Mac App Store. Una volta installata, { -app-name } le passa direttamente i tuoi file.
zynk-get-windows = Scaricala dal Microsoft Store. Una volta installata, { -app-name } le passa direttamente i tuoi file.
zynk-get-linux = Zynk per Linux è un’AppImage, senza niente da installare: scaricala, mettila in ~/Applications e rendila eseguibile (nelle sue Proprietà o con chmod +x). { -app-name } la trova lì, e ovunque altrove mentre Zynk è in esecuzione.
zynk-get = Ottieni Zynk
zynk-send-old-windows = «Invia con Zynk» richiede Windows 10 versione 2004 o successiva. «Condividi con Zynk» funziona su questo Windows.
settings-file-export-done = Impostazioni salvate in { $file }
settings-file-export-failed = Impossibile salvare le impostazioni: { $reason }
settings-file-import-unreadable = Impossibile leggere { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [one] { $count } modifica importata
       *[other] { $count } modifiche importate
    }
