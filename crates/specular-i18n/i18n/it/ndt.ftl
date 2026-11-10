# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Ispeziona
ndt-probe = Sonda: { $name }
ndt-probe-profile = Profilo lineare
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = IQI duplex a fili
ndt-probe-wire-iqi = IQI a filo singolo
ndt-probe-crack = Larghezza della cricca
ndt-probe-density = Punto di densità

ndt-hint-profile = Trascina una linea per mostrare i valori di grigio lungo di essa. [ e ] cambiano sonda.
ndt-hint-snr = Traccia una regione in un’area uniforme. Viene ingrandita ad almeno 20 × 55 pixel.
ndt-hint-duplex-iqi = Traccia una linea su tutte le coppie di fili, lungo l’IQI.
ndt-hint-wire-iqi = Traccia una linea corta attraverso il filo più sottile visibile.
ndt-hint-crack = Fai clic lungo la cricca, poi premi Invio. La larghezza viene letta in 10 punti.
ndt-hint-density = Fai clic su un punto per leggerne il valore di grigio. Digita la sua densità nota per calibrare.

ndt-refused-needs-scale = Imposta prima la scala: le dimensioni dei fili richiedono i millimetri. Usa Calibra su una lunghezza nota.
ndt-refused-nothing-found = Lungo questa linea nulla emerge dal rumore.
ndt-refused-too-small = L’immagine è più piccola della regione SNR di 20 × 55 pixel.
ndt-refused-no-noise = Questa regione non ha rumore da misurare (tutti i pixel sono uguali).

ndt-reading = Lettura…
ndt-no-pixels = Impossibile leggere i pixel.

ndt-profile-range = Grigio da { $min } a { $max }
ndt-profile-mean = Grigio medio { $mean }
ndt-profile-density = Densità da { $min } a { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = L’SNRn richiede la SRb: misura un IQI duplex o digitala.
ndt-snr-stats = Media { $mean }, σ { $sd }, regione { $w } × { $h } px

ndt-duplex-result = Prima coppia non risolta { $pair }: SRb { $srb } µm, mancanza di nitidezza { $unsharpness }
ndt-duplex-all-resolved = Tutte le coppie trovate sono risolte.
ndt-duplex-dips = Minimi: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpolato: iSRb { $isrb } µm, mancanza di nitidezza { $unsharpness }

ndt-wire-result = Filo { $wire }, { $diameter }
ndt-wire-measured = Larghezza misurata { $width } (FWHM), circa { $diameter } di diametro

ndt-crack-mean = Larghezza media { $width }
ndt-crack-range = Minima { $min }, massima { $max }
ndt-crack-cuts = Punti misurati: { $count }

ndt-density-value = Grigio { $grey }, densità { $density }
ndt-density-grey = Grigio { $grey }
ndt-density-uncalibrated = Densità non ancora calibrata: digita le densità note in due punti.

ndt-ask-density = Densità nota qui: { $value } (Invio applica, Invio senza valore salta)
ndt-ask-srb = Risoluzione spaziale di base SRb: { $value } µm (Invio applica, Esc salta)
ndt-bad-density = Digita una densità da 0 a 10.
ndt-bad-srb = Digita la SRb in micrometri, per esempio 80.
ndt-srb-set = SRb impostata a { $srb } µm per questa immagine.
ndt-density-one-more = Punto di densità registrato. Digita la densità in un altro punto.
ndt-density-calibrated = Densità calibrata da due punti.
ndt-sidecar-failed = Impossibile salvare le impostazioni di ispezione accanto al file: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m sopra il punto di decollo
map-export-kml = Salva le foto come KMZ o KML…
map-export-kmz-full = Salva le foto come KMZ con le foto complete…
map-kml-heading = Direzione { $degrees }°
map-kml-height = { $metres } m sopra il punto di decollo
map-kml-nothing = Nessuna foto con una posizione da salvare.
map-kml-saved = Foto salvate in { $name }: { $count }.
map-kml-failed = Impossibile salvare il file della mappa: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Rapporto di ispezione
inspection-report-default-title = Rapporto di ispezione
inspection-report-folder = Aggiungi le altre foto di questa cartella
inspection-report-docx = Salva anche un documento Word (DOCX)
inspection-report-kmz = Salva anche un KMZ dei luoghi delle foto
inspection-report-caption = Foto con le misure
inspection-report-col-value = Valore
inspection-report-col-scale = Scala da
inspection-report-docx-failed = Impossibile scrivere il documento Word.
inspection-report-unreadable = Non è stato possibile leggere nessuna delle foto.
