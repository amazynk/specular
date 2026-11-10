# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Inspecteren
ndt-probe = Sonde: { $name }
ndt-probe-profile = Lijnprofiel
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = Duplexdraad-IQI
ndt-probe-wire-iqi = Enkeldraads-IQI
ndt-probe-crack = Scheurbreedte
ndt-probe-density = Dichtheidspunt

ndt-hint-profile = Sleep een lijn om de grijswaarden langs die lijn te tonen. [ en ] wisselen de sonde.
ndt-hint-snr = Sleep een gebied in een egale zone. Het wordt vergroot tot minstens 20 × 55 pixels.
ndt-hint-duplex-iqi = Sleep een lijn over alle draadparen, langs de IQI.
ndt-hint-wire-iqi = Sleep een korte lijn dwars over de dunste draad die u kunt zien.
ndt-hint-crack = Klik langs de scheur en druk dan op Enter. De breedte wordt op 10 punten gemeten.
ndt-hint-density = Klik op een punt om de grijswaarde te lezen. Typ de bekende dichtheid om te kalibreren.

ndt-refused-needs-scale = Stel eerst de schaal in: draaddiameters vragen millimeters. Gebruik Kalibreren op een bekende lengte.
ndt-refused-nothing-found = Langs deze lijn steekt niets boven de ruis uit.
ndt-refused-too-small = Het beeld is kleiner dan het SNR-gebied van 20 × 55 pixels.
ndt-refused-no-noise = Dit gebied heeft geen ruis om te meten (alle pixels zijn gelijk).

ndt-reading = Lezen…
ndt-no-pixels = De pixels konden niet worden gelezen.

ndt-profile-range = Grijs { $min } tot { $max }
ndt-profile-mean = Gemiddeld grijs { $mean }
ndt-profile-density = Dichtheid { $min } tot { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = SNRn heeft de SRb nodig: meet een duplex-IQI of typ de waarde.
ndt-snr-stats = Gemiddelde { $mean }, σ { $sd }, gebied { $w } × { $h } px

ndt-duplex-result = Eerste niet-gescheiden paar { $pair }: SRb { $srb } µm, onscherpte { $unsharpness }
ndt-duplex-all-resolved = Alle gevonden paren zijn gescheiden.
ndt-duplex-dips = Dalen: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Geïnterpoleerd: iSRb { $isrb } µm, onscherpte { $unsharpness }

ndt-wire-result = Draad { $wire }, { $diameter }
ndt-wire-measured = Gemeten breedte { $width } (FWHM), ongeveer { $diameter } diameter

ndt-crack-mean = Gemiddelde breedte { $width }
ndt-crack-range = Smalst { $min }, breedst { $max }
ndt-crack-cuts = Meetpunten: { $count }

ndt-density-value = Grijs { $grey }, dichtheid { $density }
ndt-density-grey = Grijs { $grey }
ndt-density-uncalibrated = Nog geen dichtheidskalibratie: typ bekende dichtheden op twee punten.

ndt-ask-density = Bekende dichtheid hier: { $value } (Enter past toe, Enter zonder waarde slaat over)
ndt-ask-srb = Basis-ruimtelijke resolutie SRb: { $value } µm (Enter past toe, Esc slaat over)
ndt-bad-density = Typ een dichtheid van 0 tot 10.
ndt-bad-srb = Typ de SRb in micrometer, bijvoorbeeld 80.
ndt-srb-set = SRb voor dit beeld ingesteld op { $srb } µm.
ndt-density-one-more = Dichtheidspunt bewaard. Typ de dichtheid op nog één punt.
ndt-density-calibrated = Dichtheid gekalibreerd uit twee punten.
ndt-sidecar-failed = De inspectie-instellingen konden niet naast het bestand worden opgeslagen: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m boven het startpunt
map-export-kml = Foto’s opslaan als KMZ of KML…
map-export-kmz-full = Foto’s opslaan als KMZ met volledige foto’s…
map-kml-heading = Koers { $degrees }°
map-kml-height = { $metres } m boven het startpunt
map-kml-nothing = Geen foto’s met een positie om op te slaan.
map-kml-saved = Foto’s opgeslagen in { $name }: { $count }.
map-kml-failed = Het kaartbestand kon niet worden opgeslagen: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Inspectierapport
inspection-report-default-title = Inspectierapport
inspection-report-folder = De andere foto’s in deze map toevoegen
inspection-report-docx = Ook een Word-document (DOCX) opslaan
inspection-report-kmz = Ook een KMZ van de fotolocaties opslaan
inspection-report-caption = Foto met de metingen
inspection-report-col-value = Waarde
inspection-report-col-scale = Schaal uit
inspection-report-docx-failed = Het Word-document kon niet worden geschreven.
inspection-report-unreadable = Geen van de foto’s kon worden gelezen.
