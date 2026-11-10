# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Tarkasta
ndt-probe = Mittain: { $name }
ndt-probe-profile = Viivaprofiili
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = Kaksoislanka-IQI
ndt-probe-wire-iqi = Yksilanka-IQI
ndt-probe-crack = Särön leveys
ndt-probe-density = Tummumapiste

ndt-hint-profile = Vedä viiva, niin sen harmaasävyarvot piirretään käyräksi. [ ja ] vaihtavat mittaimen.
ndt-hint-snr = Vedä alue tasaiselle kohdalle. Se kasvaa vähintään kokoon 20 × 55 pikseliä.
ndt-hint-duplex-iqi = Vedä viiva kaikkien lankaparien yli IQI:n suuntaisesti.
ndt-hint-wire-iqi = Vedä lyhyt viiva ohuimman näkyvän langan poikki.
ndt-hint-crack = Napsauta säröä pitkin ja paina sitten Enter. Leveys luetaan 10 kohdasta.
ndt-hint-density = Napsauta pistettä lukeaksesi sen harmaasävyarvon. Kalibroi kirjoittamalla sen tunnettu tummuma.

ndt-refused-needs-scale = Aseta ensin mittakaava: lankojen koot vaativat millimetrit. Käytä Kalibroi-toimintoa tunnettuun pituuteen.
ndt-refused-nothing-found = Mikään ei erotu kohinasta tällä viivalla.
ndt-refused-too-small = Kuva on pienempi kuin 20 × 55 pikselin SNR-alue.
ndt-refused-no-noise = Alueella ei ole mitattavaa kohinaa (kaikki pikselit ovat samanlaisia).

ndt-reading = Luetaan…
ndt-no-pixels = Pikseleitä ei voitu lukea.

ndt-profile-range = Harmaa { $min }–{ $max }
ndt-profile-mean = Harmaan keskiarvo { $mean }
ndt-profile-density = Tummuma { $min }–{ $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = SNRn tarvitsee SRb:n: mittaa kaksoislanka-IQI tai kirjoita arvo.
ndt-snr-stats = Keskiarvo { $mean }, σ { $sd }, alue { $w } × { $h } px

ndt-duplex-result = Ensimmäinen erottumaton pari { $pair }: SRb { $srb } µm, epäterävyys { $unsharpness }
ndt-duplex-all-resolved = Kaikki löydetyt parit erottuvat.
ndt-duplex-dips = Notkot: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpoloitu: iSRb { $isrb } µm, epäterävyys { $unsharpness }

ndt-wire-result = Lanka { $wire }, { $diameter }
ndt-wire-measured = Mitattu leveys { $width } (FWHM), halkaisija noin { $diameter }

ndt-crack-mean = Keskileveys { $width }
ndt-crack-range = Kapein { $min }, levein { $max }
ndt-crack-cuts = Mittauskohtia: { $count }

ndt-density-value = Harmaa { $grey }, tummuma { $density }
ndt-density-grey = Harmaa { $grey }
ndt-density-uncalibrated = Tummumaa ei ole vielä kalibroitu: kirjoita tunnetut tummumat kahteen pisteeseen.

ndt-ask-density = Tunnettu tummuma tässä: { $value } (Enter käyttää, tyhjä Enter ohittaa)
ndt-ask-srb = Perusresoluutio SRb: { $value } µm (Enter käyttää, Esc ohittaa)
ndt-bad-density = Kirjoita tummuma väliltä 0–10.
ndt-bad-srb = Kirjoita SRb mikrometreinä, esimerkiksi 80.
ndt-srb-set = SRb asetettu tälle kuvalle: { $srb } µm.
ndt-density-one-more = Tummumapiste tallennettu. Kirjoita tummuma vielä yhteen pisteeseen.
ndt-density-calibrated = Tummuma kalibroitu kahdesta pisteestä.
ndt-sidecar-failed = Tarkastusasetuksia ei voitu tallentaa tiedoston viereen: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m lähtöpaikan yläpuolella
map-export-kml = Tallenna kuvat KMZ- tai KML-muodossa…
map-export-kmz-full = Tallenna kuvat KMZ-muodossa täysikokoisina…
map-kml-heading = Suunta { $degrees }°
map-kml-height = { $metres } m lähtöpaikan yläpuolella
map-kml-nothing = Tallennettavia kuvia, joilla on sijainti, ei ole.
map-kml-saved = Kuvia tallennettu tiedostoon { $name }: { $count }.
map-kml-failed = Karttatiedostoa ei voitu tallentaa: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Tarkastusraportti
inspection-report-default-title = Tarkastusraportti
inspection-report-folder = Lisää tämän kansion muut kuvat
inspection-report-docx = Tallenna myös Word-asiakirja (DOCX)
inspection-report-kmz = Tallenna myös KMZ kuvien paikoista
inspection-report-caption = Kuva mittauksineen
inspection-report-col-value = Arvo
inspection-report-col-scale = Mittakaavan lähde
inspection-report-docx-failed = Word-asiakirjaa ei voitu kirjoittaa.
inspection-report-unreadable = Yhtäkään kuvaa ei voitu lukea.
