# Develop: tone zones, batch conflicts, RAW+JPEG pairs, CMYK sources.
# machine-drafted, needs native review

develop-tone-zones = Zone tonali
develop-tone-zone = { $ev } EV
develop-tone-zones-adjust = Regola le zone sull’immagine
develop-tone-zones-done = Fine regolazione zone
develop-tone-zones-adjusting = Trascina in su o in giù sull’immagine per alzare o abbassare quel tono. Esc termina.
develop-tone-zone-picked = Zona { $zone }
develop-batch-conflict-title = { $count ->
    [one] Una foto è cambiata fuori da Specular
   *[other] { $count } foto sono cambiate fuori da Specular
}
develop-batch-conflict-detail = I loro file sidecar sono più recenti delle copie nella libreria.
develop-batch-conflict-keep = Mantieni: l’incolla va sopra le modifiche del file.
develop-batch-conflict-replace = Sostituisci: l’incolla va sulla copia della libreria. Le modifiche del file si perdono.
develop-batch-conflict-skip-detail = Salta: quelle foto restano come sono.
develop-batch-conflict-file = Mantieni le modifiche del file
develop-batch-conflict-library = Sostituiscile
develop-batch-conflict-skip = Salta quelle foto
develop-pair-raw = Sviluppo di { $file }, il RAW di questa coppia
develop-source-cmyk = Questa foto CMYK si modifica in RGB. È stata convertita con il suo profilo.
