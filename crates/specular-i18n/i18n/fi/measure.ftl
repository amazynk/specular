# Specular — Mittaustyökalu (pituudet, kulmat, alueet ja niiden tilastot missä tahansa kuvassa). Konekäännös, vaatii natiivitarkistuksen.

palette-measure = Mittaa
palette-measure-copy-csv = Kopioi mittaukset CSV:nä
palette-measure-export = Vie mittaukset…
toast-copied-measurements = Mittaukset kopioitu CSV:nä

measure-tool-title = Mittaa
measure-tool-list = Mittaukset
measure-tool-count = { $count ->
    [0] Ei mittauksia
    [one] Yksi mittaus
   *[other] { $count } mittausta
}
measure-tool-empty = Valitse työkalu ja piirrä kuvaan.

measure-tool-point = Piste
measure-tool-line = Viiva
measure-tool-polyline = Murtoviiva
measure-tool-angle = Kulma
measure-tool-rect = Suorakulmio
measure-tool-ellipse = Ellipsi
measure-tool-polygon = Monikulmio
measure-tool-freehand = Vapaa käsi
measure-tool-calibrate = Kalibroi
# machine-drafted, needs native review
measure-tool-spot = Mittauspiste

measure-tool-undo = Kumoa
measure-tool-delete = Poista
measure-tool-copy = Kopioi CSV
measure-tool-export = Vie…
measure-tool-import = Tuo…
measure-tool-done = Valmis
measure-tool-unit = Yksikkö: { $unit }
measure-tool-apply = Käytä

measure-tool-hint-line = Vedä tai napsauta kahta pistettä. Nuolet siirtävät hiusristikkoa, välilyönti asettaa pisteen.
measure-tool-hint-angle = Napsauta kolmea pistettä; toinen on kärki.
measure-tool-hint-rect = Vedä tai napsauta kahta kulmaa.
measure-tool-hint-ellipse = Vedä tai napsauta sen rajauslaatikon kahta kulmaa.
measure-tool-hint-polygon = Napsauta kulmia; Enter tai napsautus ensimmäiseen kulmaan sulkee sen.
measure-tool-hint-calibrate = Piirrä viiva jonkin tunnetun pituisen kohteen yli.
# machine-drafted, needs native review
measure-tool-hint-spot = Napsauta pistettä lukeaksesi sen lämpötilan.
measure-tool-known-length = Tunnettu pituus: { $value } { $unit } (kirjoita luku, U vaihtaa yksikön, Enter käyttää)
measure-tool-calibrated = Kalibroitu: yksi pikseli on { $size } { $unit }
measure-tool-bad-length = Kirjoita nollaa suurempi pituus.

measure-tool-item = { $n }. { $shape }
measure-tool-area = Pinta-ala { $value }
measure-tool-mean = Keskiarvo { $mean } ± { $sd }
measure-tool-means = Keskiarvo { $values }
measure-tool-range = Vaihteluväli { $min } – { $max }, { $count } px
measure-tool-pixels = { $count } px
measure-tool-stats-pending = Luetaan pikseleitä…
measure-tool-stats-none = Ei pikseliarvoja tässä
measure-tool-approximate = Pienennetystä kopiosta (likimääräinen)

measure-tool-deleted = Mittaus poistettu
measure-tool-undone = Kumottu
measure-tool-nothing = Tässä kuvassa ei ole mittauksia
measure-tool-exported = Mittaukset tallennettu: { $name }
measure-tool-export-failed = Mittauksia ei voitu tallentaa: { $reason }
measure-tool-imported = { $count ->
    [one] Yksi mittaus tuotu.
   *[other] { $count } mittausta tuotu.
}
measure-tool-skipped = { $count ->
    [one] Yhtä muotoa ei voitu lukea.
   *[other] { $count } muotoa ei voitu lukea.
}
measure-tool-import-failed = Mittauksia ei voitu lukea: { $reason }
measure-tool-too-large = Tiedosto on liian suuri.
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR vaatii DICOM-kuvan. Tallenna sen sijaan CSV- tai GeoJSON-muodossa.

measure-source-uncalibrated = Pikselit (ei kalibroitu)
measure-source-dicom-pixel-spacing = DICOM-pikseliväli
measure-source-dicom-calibrated = DICOM-kalibroitu pikseliväli
measure-source-dicom-imager-pixel-spacing = DICOM-ilmaisimen pikseliväli (ilmaisimella)
measure-source-dicom-magnification = DICOM-pikseliväli, suurennus korjattu
measure-source-dicom-ultrasound-region = DICOM-ultraäänialue
measure-source-tiff-resolution = TIFF-resoluutio
measure-source-exif-resolution = EXIF-resoluutio
measure-source-ome-physical-size = OME-fyysinen koko
measure-source-fits-wcs = FITS-maailmankoordinaatit
measure-source-vendor-tag = Valmistajan metatiedot
measure-source-drawing-units = Piirustusyksiköt
# machine-drafted, needs native review
measure-source-drawing-scale = Piirustuksen mittakaavasta
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = Piirustuksesta (mittakaava { $scale })
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = Piirustuksesta (mittakaava { $scale }, näkymä { $viewport })
# machine-drafted, needs native review
measure-source-map-scale = Kartan mittakaava keskellä (likimääräinen; karttatyökalut mittaavat maapallon pinnalla)
measure-source-user = Oma kalibrointisi
