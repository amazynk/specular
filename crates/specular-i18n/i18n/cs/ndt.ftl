# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = Kontrolovat
ndt-probe = Sonda: { $name }
ndt-probe-profile = Profil podél čáry
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = Duplexní drátková měrka IQI
ndt-probe-wire-iqi = Drátková měrka IQI
ndt-probe-crack = Šířka trhliny
ndt-probe-density = Bod hustoty

ndt-hint-profile = Táhněte čáru a vykreslí se hodnoty šedi podél ní. [ a ] přepínají sondu.
ndt-hint-snr = Táhněte oblast v rovnoměrné ploše. Zvětší se nejméně na 20 × 55 pixelů.
ndt-hint-duplex-iqi = Táhněte čáru přes všechny páry drátků, podél měrky IQI.
ndt-hint-wire-iqi = Táhněte krátkou čáru napříč nejtenčím drátkem, který vidíte.
ndt-hint-crack = Klikejte podél trhliny a pak stiskněte Enter. Šířka se odečte v 10 bodech.
ndt-hint-density = Klikněte na bod a odečte se jeho hodnota šedi. Pro kalibraci napište jeho známou hustotu.

ndt-refused-needs-scale = Nejdřív nastavte měřítko: velikosti drátků vyžadují milimetry. Použijte Kalibrovat na známé délce.
ndt-refused-nothing-found = Podél této čáry nic nevystupuje ze šumu.
ndt-refused-too-small = Obrázek je menší než oblast SNR 20 × 55 pixelů.
ndt-refused-no-noise = Tato oblast nemá žádný šum k měření (všechny pixely jsou stejné).

ndt-reading = Čtení…
ndt-no-pixels = Pixely nelze přečíst.

ndt-profile-range = Šeď { $min } až { $max }
ndt-profile-mean = Průměrná šeď { $mean }
ndt-profile-density = Hustota { $min } až { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = SNRn potřebuje SRb: změřte duplexní měrku IQI nebo hodnotu napište.
ndt-snr-stats = Průměr { $mean }, σ { $sd }, oblast { $w } × { $h } px

ndt-duplex-result = První nerozlišený pár { $pair }: SRb { $srb } µm, neostrost { $unsharpness }
ndt-duplex-all-resolved = Všechny nalezené páry jsou rozlišené.
ndt-duplex-dips = Poklesy: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = Interpolováno: iSRb { $isrb } µm, neostrost { $unsharpness }

ndt-wire-result = Drátek { $wire }, { $diameter }
ndt-wire-measured = Naměřená šířka { $width } (FWHM), průměr asi { $diameter }

ndt-crack-mean = Průměrná šířka { $width }
ndt-crack-range = Nejužší { $min }, nejširší { $max }
ndt-crack-cuts = Počet měřených bodů: { $count }

ndt-density-value = Šeď { $grey }, hustota { $density }
ndt-density-grey = Šeď { $grey }
ndt-density-uncalibrated = Hustota zatím není kalibrovaná: napište známé hustoty ve dvou bodech.

ndt-ask-density = Známá hustota zde: { $value } (Enter použije, prázdný Enter přeskočí)
ndt-ask-srb = Základní prostorové rozlišení SRb: { $value } µm (Enter použije, Esc přeskočí)
ndt-bad-density = Zadejte hustotu od 0 do 10.
ndt-bad-srb = Zadejte SRb v mikrometrech, například 80.
ndt-srb-set = SRb pro tento obrázek nastaveno na { $srb } µm.
ndt-density-one-more = Bod hustoty uložen. Napište hustotu ještě v jednom bodě.
ndt-density-calibrated = Hustota kalibrována ze dvou bodů.
ndt-sidecar-failed = Nastavení kontroly nelze uložit vedle souboru: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } m nad místem vzletu
map-export-kml = Uložit fotky jako KMZ nebo KML…
map-export-kmz-full = Uložit fotky jako KMZ s plnými fotkami…
map-kml-heading = Směr { $degrees }°
map-kml-height = { $metres } m nad místem vzletu
map-kml-nothing = Žádné fotky s polohou k uložení.
map-kml-saved = Počet fotek uložených do { $name }: { $count }.
map-kml-failed = Soubor mapy nelze uložit: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = Zpráva z kontroly
inspection-report-default-title = Zpráva z kontroly
inspection-report-folder = Přidat ostatní fotky z této složky
inspection-report-docx = Uložit také dokument Word (DOCX)
inspection-report-kmz = Uložit také KMZ s místy fotek
inspection-report-caption = Fotka s měřeními
inspection-report-col-value = Hodnota
inspection-report-col-scale = Měřítko podle
inspection-report-docx-failed = Dokument Word nelze zapsat.
inspection-report-unreadable = Žádnou z fotek nelze přečíst.
