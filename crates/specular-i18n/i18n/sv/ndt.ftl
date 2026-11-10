# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Granska
ndt-probe = Sond: { $name }
ndt-probe-profile = Linjeprofil
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = Dubbeltråds-IQI
ndt-probe-wire-iqi = Entråds-IQI
ndt-probe-crack = Sprickbredd
ndt-probe-density = Densitetspunkt

ndt-hint-profile = Dra en linje för att visa gråvärdena längs den. [ och ] byter sond.
ndt-hint-snr = Dra ett område på en jämn yta. Det utökas till minst 20 × 55 pixlar.
ndt-hint-duplex-iqi = Dra en linje över alla trådpar, längs IQI:n.
ndt-hint-wire-iqi = Dra en kort linje tvärs över den tunnaste tråd du kan se.
ndt-hint-crack = Klicka längs sprickan och tryck sedan på Retur. Bredden läses av i 10 punkter.
ndt-hint-density = Klicka på en punkt för att läsa av dess gråvärde. Skriv dess kända densitet för att kalibrera.

ndt-refused-needs-scale = Ange skalan först: trådstorlekar kräver millimeter. Använd Kalibrera på en känd längd.
ndt-refused-nothing-found = Inget sticker ut ur bruset längs den här linjen.
ndt-refused-too-small = Bilden är mindre än SNR-området på 20 × 55 pixlar.
ndt-refused-no-noise = Området har inget brus att mäta (alla pixlar är lika).

ndt-reading = Läser…
ndt-no-pixels = Pixlarna kunde inte läsas.

ndt-profile-range = Gråvärde { $min } till { $max }
ndt-profile-mean = Medelgråvärde { $mean }
ndt-profile-density = Densitet { $min } till { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = SNRn kräver SRb: mät en dubbeltråds-IQI eller skriv värdet.
ndt-snr-stats = Medel { $mean }, σ { $sd }, område { $w } × { $h } px

ndt-duplex-result = Första ej upplösta par { $pair }: SRb { $srb } µm, oskärpa { $unsharpness }
ndt-duplex-all-resolved = Alla hittade par är upplösta.
ndt-duplex-dips = Dalar: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpolerat: iSRb { $isrb } µm, oskärpa { $unsharpness }

ndt-wire-result = Tråd { $wire }, { $diameter }
ndt-wire-measured = Uppmätt bredd { $width } (FWHM), cirka { $diameter } i diameter

ndt-crack-mean = Medelbredd { $width }
ndt-crack-range = Smalast { $min }, bredast { $max }
ndt-crack-cuts = Mätpunkter: { $count }

ndt-density-value = Gråvärde { $grey }, densitet { $density }
ndt-density-grey = Gråvärde { $grey }
ndt-density-uncalibrated = Ingen densitetskalibrering än: skriv kända densiteter i två punkter.

ndt-ask-density = Känd densitet här: { $value } (Retur verkställer, Retur utan värde hoppar över)
ndt-ask-srb = Grundläggande spatial upplösning SRb: { $value } µm (Retur verkställer, Esc hoppar över)
ndt-bad-density = Skriv en densitet från 0 till 10.
ndt-bad-srb = Skriv SRb i mikrometer, till exempel 80.
ndt-srb-set = SRb satt till { $srb } µm för den här bilden.
ndt-density-one-more = Densitetspunkten sparad. Skriv densiteten i ytterligare en punkt.
ndt-density-calibrated = Densiteten kalibrerad från två punkter.
ndt-sidecar-failed = Granskningsinställningarna kunde inte sparas bredvid filen: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m över startplatsen
map-export-kml = Spara foton som KMZ eller KML…
map-export-kmz-full = Spara foton som KMZ med foton i full storlek…
map-kml-heading = Kurs { $degrees }°
map-kml-height = { $metres } m över startplatsen
map-kml-nothing = Inga foton med position att spara.
map-kml-saved = Foton sparade i { $name }: { $count }.
map-kml-failed = Kartfilen kunde inte sparas: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Inspektionsrapport
inspection-report-default-title = Inspektionsrapport
inspection-report-folder = Lägg till de andra fotona i den här mappen
inspection-report-docx = Spara även ett Word-dokument (DOCX)
inspection-report-kmz = Spara även en KMZ med fotonas platser
inspection-report-caption = Foto med mätningarna
inspection-report-col-value = Värde
inspection-report-col-scale = Skala från
inspection-report-docx-failed = Word-dokumentet kunde inte skrivas.
inspection-report-unreadable = Inget av fotona kunde läsas.
