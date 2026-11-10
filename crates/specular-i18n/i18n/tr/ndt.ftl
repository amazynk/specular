# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = İncele
ndt-probe = Sonda: { $name }
ndt-probe-profile = Çizgi profili
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = Çift telli IQI
ndt-probe-wire-iqi = Tek telli IQI
ndt-probe-crack = Çatlak genişliği
ndt-probe-density = Yoğunluk noktası

ndt-hint-profile = Boyunca gri değerleri çizmek için bir çizgi sürükleyin. [ ve ] sondayı değiştirir.
ndt-hint-snr = Düz bir alanda bir bölge sürükleyin. Bölge en az 20 × 55 piksele büyür.
ndt-hint-duplex-iqi = IQI boyunca, tüm tel çiftlerinin üzerinden bir çizgi sürükleyin.
ndt-hint-wire-iqi = Görebildiğiniz en ince telin üzerinden kısa bir çizgi sürükleyin.
ndt-hint-crack = Çatlak boyunca tıklayın, sonra Enter’a basın. Genişlik 10 noktada okunur.
ndt-hint-density = Gri değerini okumak için bir noktaya tıklayın. Kalibre etmek için bilinen yoğunluğunu yazın.

ndt-refused-needs-scale = Önce ölçeği ayarlayın: tel boyutları milimetre gerektirir. Bilinen bir uzunlukta Kalibre et’i kullanın.
ndt-refused-nothing-found = Bu çizgi boyunca gürültüden ayrışan bir şey yok.
ndt-refused-too-small = Görüntü, 20 × 55 piksellik SNR bölgesinden küçük.
ndt-refused-no-noise = Bu bölgede ölçülecek gürültü yok (tüm pikseller aynı).

ndt-reading = Okunuyor…
ndt-no-pixels = Pikseller okunamadı.

ndt-profile-range = Gri { $min } – { $max }
ndt-profile-mean = Ortalama gri { $mean }
ndt-profile-density = Yoğunluk { $min } – { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = SNRn için SRb gerekir: çift telli bir IQI ölçün veya değeri yazın.
ndt-snr-stats = Ortalama { $mean }, σ { $sd }, bölge { $w } × { $h } px

ndt-duplex-result = Çözümlenmemiş ilk çift { $pair }: SRb { $srb } µm, keskinsizlik { $unsharpness }
ndt-duplex-all-resolved = Bulunan tüm çiftler çözümlendi.
ndt-duplex-dips = Çukurlar: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = İnterpolasyonlu: iSRb { $isrb } µm, keskinsizlik { $unsharpness }

ndt-wire-result = Tel { $wire }, { $diameter }
ndt-wire-measured = Ölçülen genişlik { $width } (FWHM), çapı yaklaşık { $diameter }

ndt-crack-mean = Ortalama genişlik { $width }
ndt-crack-range = En dar { $min }, en geniş { $max }
ndt-crack-cuts = Ölçülen nokta sayısı: { $count }

ndt-density-value = Gri { $grey }, yoğunluk { $density }
ndt-density-grey = Gri { $grey }
ndt-density-uncalibrated = Henüz yoğunluk kalibrasyonu yok: iki noktada bilinen yoğunlukları yazın.

ndt-ask-density = Buradaki bilinen yoğunluk: { $value } (Enter uygular, boşken Enter atlar)
ndt-ask-srb = Temel uzamsal çözünürlük SRb: { $value } µm (Enter uygular, Esc atlar)
ndt-bad-density = 0 ile 10 arasında bir yoğunluk yazın.
ndt-bad-srb = SRb’yi mikrometre olarak yazın, örneğin 80.
ndt-srb-set = Bu görüntü için SRb { $srb } µm olarak ayarlandı.
ndt-density-one-more = Yoğunluk noktası tutuldu. Bir noktada daha yoğunluğu yazın.
ndt-density-calibrated = Yoğunluk iki noktadan kalibre edildi.
ndt-sidecar-failed = İnceleme ayarları dosyanın yanına kaydedilemedi: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = Kalkış noktasının { $metres } m üzerinde
map-export-kml = Fotoğrafları KMZ veya KML olarak kaydet…
map-export-kmz-full = Fotoğrafları tam boyutlu fotoğraflarla KMZ olarak kaydet…
map-kml-heading = Yön { $degrees }°
map-kml-height = Kalkış noktasının { $metres } m üzerinde
map-kml-nothing = Kaydedilecek konumlu fotoğraf yok.
map-kml-saved = { $name } dosyasına kaydedilen fotoğraf: { $count }.
map-kml-failed = Harita dosyası kaydedilemedi: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = İnceleme raporu
inspection-report-default-title = İnceleme raporu
inspection-report-folder = Bu klasördeki diğer fotoğrafları ekle
inspection-report-docx = Ayrıca bir Word belgesi (DOCX) kaydet
inspection-report-kmz = Ayrıca fotoğrafların yerlerini içeren bir KMZ kaydet
inspection-report-caption = Ölçümleriyle birlikte fotoğraf
inspection-report-col-value = Değer
inspection-report-col-scale = Ölçek kaynağı
inspection-report-docx-failed = Word belgesi yazılamadı.
inspection-report-unreadable = Fotoğrafların hiçbiri okunamadı.
