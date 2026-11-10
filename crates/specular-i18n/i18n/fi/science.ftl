# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. Suomi.

keys-mode-science = Tieteelliset kuvat
cheatsheet-science = Tieteelliset kuvat
cheatsheet-science-look = Venytys ja rajat
cheatsheet-science-planes = Tasot ja kanavat
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = Näytä tai piilota kanavat 1–9
cheatsheet-science-channels-10-16 = Näytä tai piilota kanavat 10–16
palette-science-stretch = Seuraava venytys
palette-science-limits = Seuraavat rajat
palette-science-next-plane = Seuraava taso
palette-science-prev-plane = Edellinen taso
palette-science-next-axis = Selaa toista akselia
palette-science-composite = Yhdistä kanavat värikuvaksi
palette-science-channel-1 = Näytä tai piilota kanava 1
palette-science-channel-2 = Näytä tai piilota kanava 2
palette-science-channel-3 = Näytä tai piilota kanava 3
palette-science-channel-4 = Näytä tai piilota kanava 4
palette-science-channel-5 = Näytä tai piilota kanava 5
palette-science-channel-6 = Näytä tai piilota kanava 6
palette-science-channel-7 = Näytä tai piilota kanava 7
palette-science-channel-8 = Näytä tai piilota kanava 8
palette-science-channel-9 = Näytä tai piilota kanava 9
# machine-drafted, needs native review
palette-science-channel-10 = Näytä tai piilota kanava 10
palette-science-channel-11 = Näytä tai piilota kanava 11
palette-science-channel-12 = Näytä tai piilota kanava 12
palette-science-channel-13 = Näytä tai piilota kanava 13
palette-science-channel-14 = Näytä tai piilota kanava 14
palette-science-channel-15 = Näytä tai piilota kanava 15
palette-science-channel-16 = Näytä tai piilota kanava 16
palette-science-grid = Näytä tai piilota koordinaattiruudukko
palette-science-reset = Automaattinen ulkoasu
palette-science-level = Tasoita korkeuskartat (vähennä keskitaso)
palette-science-wiggle = Seismiset wiggle-jäljet muuttuvalla pinta-alalla värien päällä
# machine-drafted, needs native review
palette-science-wireframe = Rakenteettoman hilan reunat (rautalankamalli) värien päällä
palette-science-import-lut = Väritaulukko tiedostosta (ImageJ .lut)…
palette-science-hillshade = Varjosta korkeusmallien maasto (rinnevarjostus)
palette-science-fft = Tason tehospektri (FFT)
palette-science-catalogue = Lisää lähdeluettelo kuvan päälle (VOTable, FITS-taulukko, CSV)…
palette-science-clear-catalogues = Poista kuvan päälle lisätyt luettelot
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
        [one] yksi lähde kuvassa
       *[other] { $count } lähdettä kuvassa
    }
science-catalogue-capped = { $name }: näytetään { $shown } kuvan { $count } lähteestä
science-catalogue-none = { $name }: ei lähteitä kuvassa
science-catalogue-failed = Ei taivaankoordinaatteja sisältävä luettelo: { $name }
science-catalogues-cleared = Luettelot poistettu
# The file dialog's filter name for catalogue files.
science-catalogue-filter = Lähdeluettelot
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = Ionikuva m/z-arvolla (massaspektrometria)…
disabled-not-science = Vain tieteellisille kuville (tähtitiede, elektroni- ja pyyhkäisyanturimikroskopia)
disabled-one-plane = Kuvassa on yksi taso
disabled-one-axis = Kuvassa ei ole muuta selattavaa akselia
disabled-no-channels = Kuvassa ei ole tällaista yhdistettävää kanavaa
disabled-no-coordinates = Kuvassa ei ole taivaankoordinaatteja
disabled-not-height-map = Vain korkeuskartoille (Gwyddion, Igor)
disabled-not-elevation = Vain yksikanavaisille georeferoiduille korkeusmalleille
disabled-not-seismic = Vain seismiselle datalle (SEG-Y, Seismic Unix)
disabled-not-mass-spec = Vain massaspektrometriakuville (imzML)
# machine-drafted, needs native review
disabled-not-ugrid = Vain rakenteettomille hiloille (UGRID-, ICON- ja MPAS-NetCDF)
disabled-not-bayer = Vain värikameroiden raakakuville (FITS Bayer-kuviolla, SER-video)
palette-debayer = Näytä värikameroiden kuvat debayeroituina tai raakoina
palette-bayer-pattern = Seuraava Bayer-kuvio tälle tiedostolle
science-debayer-on = Debayeroitu ({ $pattern })
science-debayer-off = Raaka värisuodinmatriisi
science-bayer-pattern = Bayer-kuvio { $pattern }
# machine-drafted, needs native review
palette-annotations = Näytä tai piilota annotaatiot
disabled-no-annotations = Kuvan vieressä ei ole annotaatioita (NDPA, Aperio tai ASAP XML)
science-annotations-on = Annotaatiot näkyvissä
science-annotations-off = Annotaatiot piilotettu
science-stretch-auto = Automaattinen (värikohtainen)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = Lineaarinen
science-stretch-sqrt = Neliöjuuri
science-stretch-log = Logaritminen
science-stretch-asinh = Asinh
science-stretch-histeq = Histogrammin tasoitus
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = Min/max
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = Tiedoston alue
# Toasts when the look changes.
science-stretch-toast = Venytys: { $stretch }
science-limits-toast = Rajat: { $limits }
science-reset-toast = Automaattinen ulkoasu
# An imported colour table: $name is its file name.
science-lut-toast = Väritaulukko { $name }
science-lut-failed = Ei väritaulukko: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = ImageJ-väritaulukot
science-composite-on = Kanavat yhdistetty värikuvaksi
science-composite-off = Yksi kanava harmaana
science-channel-on = { $channel } näkyvissä
science-channel-off = { $channel } piilotettu
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = Kanava { $number }
science-grid-on = Koordinaattiruudukko näkyvissä
science-grid-off = Koordinaattiruudukko piilotettu
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = Erotuskykyrenkaat näkyvissä
science-rings-off = Erotuskykyrenkaat piilotettu
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = Wiggle-jäljet käytössä (lähennä nähdäksesi ne)
science-wiggle-off = Wiggle-jäljet pois käytöstä
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = Hilan reunat käytössä
science-wireframe-off = Hilan reunat pois käytöstä
science-wireframe-dense = Hilan reunat käytössä (lähennä nähdäksesi ne)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = Tason tasoitus käytössä
science-level-off = Tason tasoitus pois käytöstä
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = Rinnevarjostus käytössä: aurinko { $azimuth }°, { $altitude }° korkeudella
science-hillshade-off = Rinnevarjostus pois käytöstä
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = Tehospektri näkyvissä
science-fft-off = Tehospektri piilotettu
science-axis-toast = Selataan: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = Arvo { $value }
science-value-unit = Arvo { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = Arvo { $value } (bitit { $bits })
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = Arvo { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = Ei dataa
s2-class-defective = Saturoitunut tai viallinen
s2-class-dark = Tummien alueiden pikselit
s2-class-cloud-shadow = Pilvien varjot
s2-class-vegetation = Kasvillisuus
s2-class-not-vegetated = Kasviton
s2-class-water = Vesi
s2-class-unclassified = Luokittelematon
s2-class-cloud-medium = Pilvi, keskisuuri todennäköisyys
s2-class-cloud-high = Pilvi, suuri todennäköisyys
s2-class-cirrus = Ohut cirrus
s2-class-snow = Lumi tai jää
s2-class-clear = Selkeä
s2-class-opaque-cloud = Läpinäkymättömät pilvet
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = RA { $ra }  Dek { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS ei tuettu: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = Luettelon merkit: ICRS → FK4 muunnettu
science-fk4-markers-galactic = Luettelon merkit: galaktinen → FK4 muunnettu
science-fk4-samp = SAMP: FK4 → ICRS muunnettu
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = Taso tasoitettu
# Shown under the look while the hillshade is on.
science-hillshaded = Rinnevarjostus: aurinko { $azimuth }°, { $altitude }° korkeudella
# Shown in the readout while the power spectrum is shown.
science-fft = Tehospektri (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ teho { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = Taajuus u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = Jakso { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = Faasi { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = Spektri kohdassa { $x }, { $y }
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (bini { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = { $count ->
        [one] { $count } ali-/ylivuotobini piilotettu (summa { $sum })
       *[other] { $count } ali-/ylivuotobiniä piilotettu (summa { $sum })
    }
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = Vaihtelevanlevyiset binit uudelleennäytteistetty tasaiseen ruudukkoon
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
    [one] { $count } tapahtuma
   *[other] { $count } tapahtumaa
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = Kompensoitu ({ $keyword })
science-cytometry-uncompensated = Ei kompensoitu
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = Inline { $inline }  Crossline { $crossline }
science-seismic-trace = Jälki { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = Ei jälkeä
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = Aika { $time } ms
science-seismic-depth = Syvyys { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = Tahko { $face }
science-mesh-face-node = Tahko { $face }  Solmu { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = Hilan ulkopuolella
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index }/{ $count }
science-plane-world = { $axis } { $index }/{ $count } ({ $world })
science-axis-unnamed = Taso
science-axis-extension = Kuva
science-axis-channel = Kanava
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = Kenttä
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = Bin
science-bin-size = { $size } px
science-axis-band = Energia
science-band-all = kaikki energiat
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = Osa
science-part-magnitude = itseisarvo
science-part-real = reaaliosa
science-part-imaginary = imaginaariosa
science-part-phase = vaihe (radiaaneina)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = intensiteetti (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = Ruutu
science-readout = Arvo ja koordinaatit
# Accessible names of the plane sliders.
science-sliders = Tasot
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = Mittakaavapalkki: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = Kokonaisionivirta
science-ion-window = Ionikuva m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = Ionikuva m/z-arvolla: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab toleranssi · Enter näytä · Esc peruuta
science-ion-bad = Ei m/z-arvo: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = Piste x { $x }  y { $y }
science-ms-spot-z = Piste x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = Tässä pisteessä ei ole spektriä
# machine-drafted, needs native review
palette-science-temperature-unit = Lämpötilan yksikkö (°C, °F, K)
palette-science-isotherm = Isotermi osoittimen kohdan lämpötilan ylä- tai alapuolella
palette-science-thermal-parameters = Emissiivisyys ja heijastunut lämpötila…
palette-science-export-temperatures = Vie lämpötilat (liukuluku-TIFF tai CSV)…
palette-thermal-visible = Lämpökuvan näkyvän valon kuva
disabled-not-thermal = Vain lämpökuville
disabled-not-radiometric = Tiedosto tallentaa lämpötiloja: sen parametreja ei voi muuttaa
disabled-no-visible-photo = Lämpökuvassa ei ole näkyvän valon kuvaa
science-temperature-unit-toast = Lämpötilat yksikössä { $unit }
science-isotherm-above = Isotermi yli { $level }
science-isotherm-below = Isotermi alle { $level }
science-isotherm-off = Isotermi pois
science-thermal-emissivity-prompt = Emissiivisyys: { $text }
science-thermal-reflected-prompt = Heijastunut lämpötila ({ $unit }): { $text }
science-thermal-prompt-hint = Tab seuraava kenttä · Enter laske uudelleen · Esc peruuta
science-thermal-bad = Ei kelvollinen arvo: { $text }
science-thermal-applied = Laskettu uudelleen emissiivisyydellä { $emissivity }, heijastunut { $reflected }
science-thermal-conditions = Emissiivisyys { $emissivity } · heijastunut { $reflected }
science-thermal-export-tiff = 32-bittinen liukuluku-TIFF
science-thermal-export-csv = CSV
science-thermal-export-done = Lämpötilat tallennettu: { $name }
science-thermal-export-failed = Lämpötilojen tallennus epäonnistui: { $reason }
# machine-drafted, needs native review
science-values-failed = Arvoja ei voitu näyttää: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = Toista tai keskeytä lämpökuvasarja
palette-science-thermal-series-csv = Tallenna lämpötilat ajan mukaan (CSV)…
disabled-not-thermal-sequence = Vain lämpökuvasarjoille
thermal-seq-title = Lämpötilat ajan mukaan
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = Ruutu { $frame }/{ $frames } · { $time }
thermal-seq-play = Toista
thermal-seq-pause = Tauko
thermal-seq-playing = Toistetaan
thermal-seq-paused = Keskeytetty
thermal-seq-save-csv = Tallenna CSV…
thermal-seq-hint = Lisää mittauspiste, viiva tai alue (M), niin sen lämpötila piirretään ajan mukaan.
thermal-seq-reading = Luetaan ruutua { $done }/{ $total }…
thermal-seq-failed = Ruutujen lukeminen epäonnistui: { $reason }
thermal-seq-no-series = Ei vielä tallennettavaa: lisää mittauspiste, viiva tai alue.
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = Kaavio: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = Raportti (PDF)…
thermal-report-heading = Lämpökuvausraportti
thermal-report-default-title = Lämpökuvaustarkastuksen raportti
thermal-report-title-label = Otsikko
thermal-report-notes-label = Muistiinpanot
thermal-report-photo = Liitä näkyvän valon kuva
thermal-report-folder = Lisää tämän kansion muut lämpökuvat
thermal-report-save = Tallenna PDF…
thermal-report-busy = Kirjoitetaan raporttia…
thermal-report-nothing = Lämpötiloja ei vielä näytetä.
thermal-report-saved = Raportti tallennettu: { $name } ({ $count ->
    [one] { $count } kuva
   *[other] { $count } kuvaa
})
thermal-report-failed = Raportin tallennus epäonnistui: { $reason }
# In the PDF.
thermal-report-notes = Muistiinpanot
thermal-report-measurements = Mittaukset
thermal-report-no-measurements = Ei mittauksia.
thermal-report-thermal-caption = Lämpökuva
thermal-report-visible-caption = Näkyvän valon kuva
thermal-report-col-measurement = Mittaus
thermal-report-col-min = Min.
thermal-report-col-max = Maks.
thermal-report-col-mean = Keskiarvo
thermal-report-col-emissivity = Emissiivisyys
thermal-report-whole-image = Koko kuva
thermal-report-camera = Kamera
thermal-report-lens = Objektiivi
thermal-report-date = Otettu
thermal-report-place = Paikka
thermal-report-frame = Ruutu
thermal-report-emissivity = Emissiivisyys
thermal-report-reflected = Heijastunut lämpötila
thermal-report-distance = Etäisyys
thermal-report-humidity = Kosteus
thermal-report-range = Alue
thermal-report-format = Muoto
thermal-report-page = Sivu { $page }/{ $pages }
