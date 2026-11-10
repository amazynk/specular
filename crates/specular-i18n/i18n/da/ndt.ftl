# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Inspicér
ndt-probe = Probe: { $name }
ndt-probe-profile = Linjeprofil
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = Dobbelttråds-IQI
ndt-probe-wire-iqi = Enkelttråds-IQI
ndt-probe-crack = Revnebredde
ndt-probe-density = Densitetspunkt

ndt-hint-profile = Træk en linje for at vise gråværdierne langs den. [ og ] skifter probe.
ndt-hint-snr = Træk et område i en jævn flade. Det udvides til mindst 20 × 55 pixels.
ndt-hint-duplex-iqi = Træk en linje hen over alle trådparrene, langs IQI’en.
ndt-hint-wire-iqi = Træk en kort linje på tværs af den tyndeste tråd, du kan se.
ndt-hint-crack = Klik langs revnen, og tryk derefter på Enter. Bredden aflæses i 10 punkter.
ndt-hint-density = Klik på et punkt for at aflæse dets gråværdi. Skriv dets kendte densitet for at kalibrere.

ndt-refused-needs-scale = Angiv først målestokken: trådstørrelser kræver millimeter. Brug Kalibrér på en kendt længde.
ndt-refused-nothing-found = Intet skiller sig ud fra støjen langs denne linje.
ndt-refused-too-small = Billedet er mindre end SNR-området på 20 × 55 pixels.
ndt-refused-no-noise = Dette område har ingen støj at måle (alle pixels er ens).

ndt-reading = Læser…
ndt-no-pixels = Pixelene kunne ikke læses.

ndt-profile-range = Gråværdi { $min } til { $max }
ndt-profile-mean = Middelgråværdi { $mean }
ndt-profile-density = Densitet { $min } til { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = SNRn kræver SRb: mål en dobbelttråds-IQI, eller skriv værdien.
ndt-snr-stats = Middel { $mean }, σ { $sd }, område { $w } × { $h } px

ndt-duplex-result = Første uopløste par { $pair }: SRb { $srb } µm, uskarphed { $unsharpness }
ndt-duplex-all-resolved = Alle fundne par er opløst.
ndt-duplex-dips = Dyk: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpoleret: iSRb { $isrb } µm, uskarphed { $unsharpness }

ndt-wire-result = Tråd { $wire }, { $diameter }
ndt-wire-measured = Målt bredde { $width } (FWHM), ca. { $diameter } i diameter

ndt-crack-mean = Middelbredde { $width }
ndt-crack-range = Smallest { $min }, bredest { $max }
ndt-crack-cuts = Målepunkter: { $count }

ndt-density-value = Gråværdi { $grey }, densitet { $density }
ndt-density-grey = Gråværdi { $grey }
ndt-density-uncalibrated = Ingen densitetskalibrering endnu: skriv kendte densiteter i to punkter.

ndt-ask-density = Kendt densitet her: { $value } (Enter anvender, Enter uden værdi springer over)
ndt-ask-srb = Grundlæggende rumlig opløsning SRb: { $value } µm (Enter anvender, Esc springer over)
ndt-bad-density = Skriv en densitet fra 0 til 10.
ndt-bad-srb = Skriv SRb i mikrometer, for eksempel 80.
ndt-srb-set = SRb sat til { $srb } µm for dette billede.
ndt-density-one-more = Densitetspunkt gemt. Skriv densiteten i ét punkt mere.
ndt-density-calibrated = Densitet kalibreret ud fra to punkter.
ndt-sidecar-failed = Inspektionsindstillingerne kunne ikke gemmes ved siden af filen: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m over startstedet
map-export-kml = Gem fotos som KMZ eller KML…
map-export-kmz-full = Gem fotos som KMZ med fotos i fuld størrelse…
map-kml-heading = Kurs { $degrees }°
map-kml-height = { $metres } m over startstedet
map-kml-nothing = Ingen fotos med en position at gemme.
map-kml-saved = Fotos gemt i { $name }: { $count }.
map-kml-failed = Kortfilen kunne ikke gemmes: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Inspektionsrapport
inspection-report-default-title = Inspektionsrapport
inspection-report-folder = Tilføj de andre fotos i denne mappe
inspection-report-docx = Gem også et Word-dokument (DOCX)
inspection-report-kmz = Gem også en KMZ med fotoenes steder
inspection-report-caption = Foto med målingerne
inspection-report-col-value = Værdi
inspection-report-col-scale = Målestok fra
inspection-report-docx-failed = Word-dokumentet kunne ikke skrives.
inspection-report-unreadable = Ingen af fotoene kunne læses.
