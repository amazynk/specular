# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = Sovrapponi regioni DS9…
palette-science-load-regions-help = Disegna sull’immagine le forme di un file di regioni DS9 (.reg).
palette-science-export-regions = Salva regioni come DS9…
palette-science-export-regions-help = Salva le misure e le regioni sovrapposte come file di regioni DS9.
palette-science-edit-regions = Modifica regioni come misure
palette-science-edit-regions-help = Trasforma le regioni sovrapposte in misure da spostare e modificare.
palette-science-clear-regions = Rimuovi le regioni sovrapposte
palette-science-clear-regions-help = Rimuovi le regioni DS9 dall’immagine.
palette-science-plate-solve = Risoluzione astrometrica
palette-science-plate-solve-help = Trova le coordinate celesti dell’immagine con ASTAP o astrometry.net. Esegui di nuovo per interrompere.
palette-science-save-plate-solution = Salva la soluzione astrometrica
palette-science-save-plate-solution-help = Salva le coordinate celesti trovate accanto all’immagine, come file .wcs.
palette-science-sky-survey = Survey del cielo come sfondo
palette-science-sky-survey-help = Mostra la survey del cielo successiva attorno all’immagine, o nessuna.
palette-science-samp-send-image = Invia immagine alle applicazioni SAMP
palette-science-samp-send-image-help = Apri questa immagine in DS9, Aladin e nelle altre applicazioni sull’hub SAMP.
palette-science-samp-send-position = Invia posizione alle applicazioni SAMP
palette-science-samp-send-position-help = Punta le altre applicazioni sulla posizione celeste sotto il puntatore.

disabled-samp-off = Connettiti prima a un hub SAMP (Impostazioni › Integrazione).

settings-integration-samp = Connetti a un hub SAMP
settings-integration-samp-help = Scambia immagini, tabelle e posizioni celesti con DS9, TOPCAT, Aladin e altre applicazioni di astronomia. Viene usato solo un hub su questo computer.
settings-integration-astap-path = Programma ASTAP
settings-integration-astap-path-help = Il risolutore astrometrico ASTAP, se non viene trovato da solo.
settings-integration-solve-field-path = Programma astrometry.net
settings-integration-solve-field-path-help = Il programma solve-field di astrometry.net, se non viene trovato da solo.

astro-regions-filter = Regioni DS9
astro-regions-loaded = { $name }: { $count ->
        [one] { $count } regione.
       *[other] { $count } regioni.
    }
astro-regions-refused = { $count ->
        [one] { $count } riga non letta.
       *[other] { $count } righe non lette.
    } Prima: riga { $line }, { $reason }.
astro-regions-approximate = { $count ->
        [one] { $count } panda disegnato con i bordi interno ed esterno.
       *[other] { $count } panda disegnati con i bordi interno ed esterno.
    }
astro-regions-none-here = { $name }: nessuna regione cade su questa immagine.
astro-regions-failed = Impossibile leggere { $name }: { $reason }
astro-regions-cleared = Regioni rimosse.
astro-regions-nothing = Nessuna regione o misura da salvare.
astro-regions-saved = { $name } salvato.
astro-regions-skipped = { $count ->
        [one] { $count } forma fuori dalle coordinate celesti è stata esclusa.
       *[other] { $count } forme fuori dalle coordinate celesti sono state escluse.
    }
astro-regions-save-failed = Impossibile salvare le regioni: { $reason }
astro-regions-edited = { $count ->
        [one] { $count } regione è ora una misura.
       *[other] { $count } regioni sono ora misure.
    }
astro-regions-no-wcs = Le regioni celesti richiedono un’immagine con coordinate celesti.

astro-solve-no-solver = Nessun risolutore astrometrico trovato. Installa ASTAP o astrometry.net, oppure scegline uno in Impostazioni › Integrazione.
astro-solve-running = Risoluzione con { $solver }… { $seconds } s
astro-solve-stop = Interrompi
astro-solve-stopped = Risoluzione astrometrica interrotta.
astro-solve-done = Risolta: { $ra }, { $dec }, { $scale } per pixel.
astro-solve-failed = Risoluzione astrometrica non riuscita: { $reason }
astro-solve-not-local = Si possono risolvere solo immagini in una cartella di questo computer.
astro-solve-none = Ancora nessuna soluzione astrometrica per questa immagine.
astro-solve-saved = { $name } salvato. L’immagine in sé non è cambiata.
astro-solve-sidecar = Coordinate celesti da { $name }.

astro-samp-connected = Connesso all’hub SAMP.
astro-samp-no-hub = SAMP: { $reason }. Nuovo tentativo quando si avvia un hub.
astro-samp-lost = L’hub SAMP si è fermato. Nuovo tentativo quando se ne avvia uno.
astro-samp-sent = { $count ->
        [one] Inviato a { $count } applicazione.
       *[other] Inviato a { $count } applicazioni.
    }
astro-samp-nobody = Nessuna applicazione sull’hub è in ascolto per questo.
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } non è su questo computer, quindi non viene aperto.
astro-samp-pointed = Puntato su { $ra }, { $dec }.
astro-samp-off-image = { $ra }, { $dec } non è su questa immagine.
astro-samp-no-table = SAMP: quella tabella non è sovrapposta qui.
astro-samp-rows = { $count ->
        [one] { $count } sorgente del catalogo evidenziata.
       *[other] { $count } sorgenti del catalogo evidenziate.
    }

astro-survey-off = Survey del cielo disattivata.
astro-survey-on = Survey del cielo: { $name }. { $credit }
astro-survey-failed = Impossibile mostrare { $name }: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = Tasselli della survey del cielo da { $host }
