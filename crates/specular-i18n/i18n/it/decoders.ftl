# machine-drafted, needs native review

settings-section-decoders = Decoder opzionali
settings-decoders-look-again = Cerca di nuovo
settings-decoders-look-again-help = Cerca i decoder installati nel frattempo.
    { -app-name } cerca anche ogni volta che la sua finestra torna in primo piano.
settings-decoders-braw = Usa il Blackmagic RAW SDK
settings-decoders-braw-help = Le clip Blackmagic RAW si aprono con esso quando è installato.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = Lascia vuoto per trovarlo dove lo mette il suo programma di installazione.
settings-decoders-dwg = Usa ODA File Converter
settings-decoders-dwg-help = I disegni DWG si aprono con esso quando è installato.
    Una copia del disegno viene convertita in una cartella privata, che poi viene eliminata.
settings-decoders-dwg-path = Programma ODA File Converter
settings-decoders-dwg-path-help = Lascia vuoto per trovarlo dove lo mette il suo programma di installazione.
settings-decoders-occt = Usa OpenCASCADE
settings-decoders-occt-help = I modelli STEP e IGES si aprono con esso quando è installato.
settings-decoders-occt-path = Librerie OpenCASCADE
settings-decoders-occt-path-help = Lascia vuoto per trovarle dove il sistema tiene le librerie.
# machine-drafted, needs native review
settings-decoders-czi = Usa libCZI
settings-decoders-czi-help = I file Zeiss CZI si aprono con esso quando è installato.
    Funziona nel processo di supporto isolato di { -app-name }, senza accesso agli altri tuoi file.
settings-decoders-czi-path = Cartella della libreria libCZI
settings-decoders-czi-path-help = Lascia vuoto per trovarla dove il sistema tiene le librerie.

decoders-intro = Alcuni formati si aprono con software gratuito dei loro produttori.
    Installalo una volta: { -app-name } lo trova e lo usa. Non lo scarica mai.
decoders-keywords = decoder opzionali produttore SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI serve decoder
decoders-row = { $format } ({ $extensions })
decoders-state-found = Trovato
decoders-state-off = Disattivato
decoders-state-not-found = Non installato
decoders-state-broken = Non utilizzabile
decoders-state-refused = Non possibile
decoders-state-planned = Non ancora disponibile
decoders-state-checking = Ricerca…
decoders-uses = Si apre con { $decoder } di { $vendor }.
decoders-not-found = È gratuito. Installalo, poi premi Cerca di nuovo.
decoders-off = Disattivato: questi file non si aprono.
decoders-version = Versione { $version }.
decoders-r3d-why = La licenza di RED consente di leggere i file R3D solo ai programmi creati con il suo SDK, alle condizioni di RED.
    { -app-name } non può accettare queste condizioni, quindi i file R3D non si aprono.
decoders-ecw-why = La licenza di Hexagon consente di usare il suo ECW SDK solo ai programmi che lo includono, non a una copia installata da te.
    Per questo i file ECW non si aprono. I file JPEG 2000 creati con l’ECW SDK sì.
decoders-mrsid-why = La licenza del MrSID SDK ne consente l’uso solo ai programmi che lo includono, non a una copia installata da te.
    Per questo i file MrSID non si aprono.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = La licenza dell’SDK di DJI ne consente l’uso solo ai programmi che lo includono, non a una copia installata da te. La foto e i suoi metadati si aprono comunque.
decoders-occt-why = I solidi STEP e IGES richiedono OpenCASCADE, che questa versione di { -app-name } non può usare.
decoders-get-row = La pagina di download del produttore
decoders-get = Scarica
decoders-locate-row = Installato altrove?
decoders-locate = Individua…
decoders-forget-row = Scelto a mano.
decoders-forget = Dimentica…
decoders-forget-ask = Dimenticare la tua scelta per { $decoder }?
decoders-forget-detail = { -app-name } torna a cercarlo da solo. Nulla viene rimosso dal disco.
decoders-forget-ok = Dimentica
decoders-locate-title = Individua { $decoder }
decoders-locate-wrong = Questo non è { $decoder }.
decoders-located = In uso la tua scelta per { $decoder }.
decoders-forgotten = { -app-name } torna a cercare { $decoder } da solo.
decoders-still-missing = { $decoder } non è ancora stato trovato.
decoders-now-found = { $decoder } trovato.
decoders-looked = Decoder cercati di nuovo.

decoders-card-title = { $format } richiede il decoder gratuito di { $vendor }
decoders-card-what = { $file } si apre con { $decoder }. È gratuito.
decoders-card-how = Installalo, poi torna qui: { -app-name } lo trova da solo.
decoders-card-get = Scarica
decoders-card-installed = L’ho installato
decoders-card-locate = Individua…

gallery-badge-needs-decoder = Serve un decoder
gallery-needs-decoder = Si apre con { $decoder } di { $vendor }, gratuito ma non installato.

error-reason-needs-decoder = I file { $format } richiedono { $decoder } di { $vendor }, gratuito ma non installato.
