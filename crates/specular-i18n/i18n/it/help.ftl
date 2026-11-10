# machine-drafted, needs native review
## Il pannello del manuale, i rapporti di arresto anomalo, il pacchetto diagnostico, gli aggiornamenti e Informazioni. Italiano.
## Stile: breve, chiaro, senza riempitivi; un'idea per frase.

## Tavolozza

palette-help-manual = Aiuto
palette-help-manual-help = Apri il manuale alla pagina di ciò che è sullo schermo.
palette-save-diagnostics = Salva pacchetto diagnostico…
palette-save-diagnostics-help = Salva rapporti di arresto anomalo, registro e dettagli di sistema in un file zip. Non viene inviato nulla.
# Voce della tavolozza che apre una sezione del manuale; { $title } è una pagina o «Pagina › Sezione».
palette-help-topic = Aiuto: { $title }
palette-help-topic-help = Apri questa sezione del manuale.
# Sotto un comando non disponibile nella tavolozza.
palette-disabled-help = Non disponibile ora. Premi Invio per sapere perché.

## Il pannello dell'aiuto

help-title = Aiuto
help-search-placeholder = Cerca nel manuale
help-contents = Sommario
help-back = Indietro
help-forward = Avanti
help-no-results = Nessun risultato per «{ $query }».
help-results = { $count ->
    [one] 1 sezione trovata
    [many] { $count } sezioni trovate
   *[other] { $count } sezioni trovate
}
help-section-start = Primi passi
help-section-modes = Visualizzazione e modalità
help-section-tasks = Come fare…
help-section-reference = Riferimento
help-keys-unbound = Non assegnato
# Un comando citato nel manuale senza tasto; si esegue dalla tavolozza.
help-key-unbound = { $name } (tavolozza)
help-keys-command = Comando
help-keys-keys = Tasti
help-commands = Ogni comando è nella tavolozza dei comandi ({ $key }). Digita parte del nome.
help-commands-no-key = Ogni comando è nella tavolozza dei comandi. Digita parte del nome.
help-link = Vai a { $title }
help-open = Aiuto
# Nome del pannello per i lettori di schermo; { $page } è la pagina mostrata.
help-label = Aiuto: { $page }
help-disabled-why = Perché?

## Rapporto di arresto anomalo proposto all'avvio successivo

crash-title = { -app-name } si è chiuso inaspettatamente
crash-body = Un rapporto è stato salvato su questo computer. Non è stato inviato nulla.
crash-view = Mostra rapporto
crash-hide = Nascondi rapporto
crash-copy = Copia
crash-copied = Rapporto copiato
crash-save-bundle = Salva pacchetto diagnostico…
crash-dismiss = Ignora

## Pacchetto diagnostico

diag-title = Salva pacchetto diagnostico
diag-intro = Un file zip da allegare a una segnalazione di bug. Viene salvato su questo computer. Non viene inviato nulla.
diag-contents = Contenuto
diag-include-names = Includi i nomi dei file
diag-include-names-help = Disattivato: i nomi sono sostituiti da file-1.jpg, dir-1 e così via.
diag-privacy = La tua cartella home appare come ~ e il tuo nome utente come <user>. Password, indirizzi dei server e immagini non sono mai inclusi.
diag-where = Segnala un problema: { $issues } · { $email }
diag-save = Salva…
diag-cancel = Annulla
diag-saved = Pacchetto diagnostico salvato: { $path }
diag-failed = Impossibile salvare il pacchetto diagnostico: { $reason }
diag-entry-summary = Versione, sistema, GPU e driver
diag-entry-crash = Ultimo rapporto di arresto anomalo
diag-entry-crashes = Rapporti di arresto anomalo precedenti ({ $count })
diag-entry-log = Ultime { $count } righe del registro
diag-entry-settings = Impostazioni, senza password né indirizzi
diag-entry-gpu = Dettagli di GPU e schermo
diag-entry-readme = Che cos'è questo pacchetto
diag-size = { $size }
# Il README.txt nel pacchetto.
diag-readme =
    Pacchetto diagnostico di { -app-name }.

    Contiene ciò che aiuta a trovare un problema: versione e dettagli di sistema, rapporti di arresto anomalo, la fine del registro e le impostazioni.

    La tua cartella home appare come ~ e il tuo nome utente come <user>. Se non hai scelto di includerli, i nomi di file e cartelle sono sostituiti da file-1, dir-1 e così via. Password e indirizzi dei server sono rimossi.

    Non è stato inviato nulla. Sei tu a decidere se condividere questo file.

## Aggiornamenti (Impostazioni)

settings-section-updates = Aggiornamenti
settings-updates-check = Cerca aggiornamenti
settings-updates-check-help = Una volta al giorno, chiedi se esiste una nuova versione. La richiesta contiene solo il numero di versione: nessun identificatore, nessun cookie. Disattivato: { -app-name } non si connette mai per gli aggiornamenti.
settings-updates-channel = Canale
settings-updates-channel-help = Stabile: solo le versioni rilasciate. Beta: anche le versioni di prova.
settings-updates-channel-stable = Stabile
settings-updates-channel-beta = Beta
settings-updates-check-now = Controlla ora
settings-updates-check-now-help = Controlla una volta ora.
update-available = È disponibile la versione { $version }
update-notes = Note di rilascio
update-download = Scarica
update-none = { -app-name } è aggiornato.
update-checking = Ricerca aggiornamenti…
update-failed = Impossibile cercare aggiornamenti: { $reason }
update-last-checked = Ultimo controllo: { $when }

## Informazioni

about-build = Build { $commit } · { $profile } · { $target }
about-build-no-commit = Build { $profile } · { $target }
about-help = Aiuto
about-diagnostics = Salva pacchetto diagnostico…
# machine-drafted, needs native review
about-renderer = Info renderer…
about-licences-count = { $count ->
    [one] 1 componente di terze parti
    [many] { $count } componenti di terze parti
   *[other] { $count } componenti di terze parti
}
about-licence-group = { $licence } ({ $count })
about-contact = Segnalazioni: { $issues } · Supporto: { $email }
