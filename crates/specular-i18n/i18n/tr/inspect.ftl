# machine-drafted, needs native review
# İnceleme araçları: kırpılma uyarıları, odak vurgulama, histogram, piksel denetçisi.

## Komut paleti
palette-clipping = Kırpılma uyarılarını göster veya gizle
palette-focus-peaking = Odak vurgulamayı göster veya gizle
palette-histogram = Histogramı göster veya gizle
palette-pixel-inspector = Piksel denetçisini göster veya gizle
# machine-drafted, needs native review
palette-crosshair = Klavye artı imleci
palette-announce-cursor = İmlecin altındaki değeri oku

## Histogram paneli
histogram-title = Histogram
# Kırpılma düzeylerinde veya ötesindeki görünür piksellerin payı; değerler yüzdedir.
histogram-clipped = Kırpılan: açık tonlar { $high }, gölgeler { $low }
histogram-empty = Görünür görüntü yok
# Ekran okuyucu özeti. $mean ortalama parlaklıktır (0–255), $high/$low yüzdedir.
histogram-a11y = Görünür görüntünün histogramı: ortalama parlaklık { $mean }, kırpılan açık tonlar { $high }, kırpılan gölgeler { $low }

## Piksel denetçisi paneli
inspector-title = Piksel denetçisi
inspector-position = Konum
inspector-source-rgb = Kaynak RGB
inspector-source-rgba = Kaynak RGBA
inspector-source-ycc = Kaynak Y′CbCr
inspector-source-gray = Kaynak gri
inspector-source-gray16 = Kaynak gri (16 bit)
inspector-source-gray-alpha16 = Kaynak gri + alfa (16 bit)
inspector-source-rgb16 = Kaynak RGB (16 bit)
inspector-source-rgba16 = Kaynak RGBA (16 bit)
inspector-source-pending = Kaynak
inspector-srgb = sRGB
inspector-hex = Onaltılık
inspector-linear = Doğrusal
# Değerler tam çözünürlüklü piksellerden değil, küçültülmüş bir önizlemeden geldiğinde gösterilir.
inspector-approximate = Küçültülmüş önizlemeden; kesin değerler için yakınlaştırın
inspector-outside = Bir pikseli incelemek için görüntünün üzerine gelin
# Ekran okuyucu özeti: piksel konumu ve sRGB değeri.
inspector-a11y = Piksel { $x }, { $y }: sRGB { $value }

## Ayarlar
settings-section-inspection = İnceleme
settings-inspection-clip-highlight = Açık ton kırpılma düzeyi
settings-inspection-clip-highlight-help = Herhangi bir kanalı bu 8 bitlik değerde veya üzerinde olan pikseller kırpılmış açık ton olarak işaretlenir.
settings-inspection-clip-shadow = Gölge kırpılma düzeyi
settings-inspection-clip-shadow-help = Herhangi bir kanalı bu 8 bitlik değerde veya altında olan pikseller kırpılmış gölge olarak işaretlenir.
settings-inspection-highlight-color = Açık ton uyarı rengi
settings-inspection-highlight-color-help = Kırpılmış açık tonları işaretleyen renk.
settings-inspection-shadow-color = Gölge uyarı rengi
settings-inspection-shadow-color-help = Kırpılmış gölgeleri işaretleyen renk.
settings-inspection-peaking-sensitivity = Odak vurgulama hassasiyeti
settings-inspection-peaking-sensitivity-help = Daha yüksek değerler daha yumuşak kenarları da odakta olarak işaretler.
settings-inspection-peaking-color = Odak vurgulama rengi
settings-inspection-peaking-color-help = Keskin kenarları işaretleyen renk.
settings-inspection-histogram-channels = Histogram kanalları
settings-inspection-histogram-channels-help = Histogramın hangi eğrileri gösterdiği.
settings-inspection-histogram-channels-colour = Kırmızı, yeşil ve mavi
settings-inspection-histogram-channels-luminance = Parlaklık
settings-inspection-histogram-channels-both = Renk ve parlaklık
settings-inspection-histogram-log = Logaritmik histogram
settings-inspection-histogram-log-help = Küçük sayıların görünür kalması için histogramı logaritmik ölçekle.

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = Kırmızı kanalı göster
palette-channel-green = Yeşil kanalı göster
palette-channel-blue = Mavi kanalı göster
palette-channel-alpha = Alfa kanalını göster (tekrar: dama tahtası üzerinde, sonra bir renk üzerinde)
palette-channel-luma = Parlaklığı göster
channel-normal = Tüm kanallar
channel-red = Kırmızı kanal
channel-green = Yeşil kanal
channel-blue = Mavi kanal
channel-alpha = Alfa kanalı
channel-alpha-checker = Dama tahtası üzerinde saydamlık
channel-alpha-matte = Renk üzerinde saydamlık
channel-luma = Parlaklık
disabled-no-alpha = Bu görüntüde saydamlık yok
settings-inspection-alpha-matte-color = Alfa arka plan rengi
settings-inspection-alpha-matte-color-help = “Renk üzerinde saydamlık” kanal görünümünde görüntünün arkasındaki düz renk.

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = Yanlış rengi göster veya gizle
palette-next-colour-map = Sonraki renk haritası
palette-channel-colours = Sonraki kanal renkleri (kırmızı–yeşil, macenta–yeşil, macenta–sarı–camgöbeği)
palette-hilo = HiLo’yu göster veya gizle (bit derinliğinin en yüksek ve en düşük değerindeki pikseller)
palette-colour-vision = Sonraki renk görme simülasyonu
palette-colour-vision-off = Renk görme simülasyonunu durdur
palette-colour-vision-protanopia = Protanopi simülasyonu (kırmızı koni yok)
palette-colour-vision-deuteranopia = Döteranopi simülasyonu (yeşil koni yok)
palette-colour-vision-tritanopia = Tritanopi simülasyonu (mavi koni yok)
palette-colour-vision-achromatopsia = Akromatopsi simülasyonu (renk yok)
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet (renk körlüğüne uygun değil)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = ham değerler
# Screen reader summary of the colour bar.
colour-bar-a11y = Renk çubuğu { $map }: { $low } ile { $high } { $unit } arası
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = Yanlış renk: { $map }
badge-hilo = HiLo: en yüksek ve en düşük değerdeki pikseller işaretleniyor
badge-colour-vision-protanopia = Protanopi simüle ediliyor
badge-colour-vision-deuteranopia = Döteranopi simüle ediliyor
badge-colour-vision-tritanopia = Tritanopi simüle ediliyor
badge-colour-vision-achromatopsia = Akromatopsi simüle ediliyor
channel-colours-red-green = Kanallar: kırmızı, yeşil, mavi
channel-colours-magenta-green = Kanallar: macenta, yeşil
channel-colours-magenta-yellow-cyan = Kanallar: macenta, sarı, camgöbeği
settings-inspection-false-colour-map = Yanlış renk haritası
settings-inspection-false-colour-map-help = Yanlış renk ve renk çubuğu için renk haritası. Jet ve phase dışındakilerin tümü her renk görmesiyle okunaklı kalır.
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (ıraksak)
settings-inspection-false-colour-map-vik = Vik (ıraksak)
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance (ıraksak)
settings-inspection-false-colour-map-curl = Curl (ıraksak)
settings-inspection-false-colour-map-phase = Phase (döngüsel)
settings-inspection-false-colour-map-jet = Jet (renk körlüğüne uygun değil)
settings-inspection-colour-bar = Renk çubuğu
settings-inspection-colour-bar-help = Yanlış renk açıkken görüntünün kendi birimlerinde değerler içeren bir renk çubuğu göster.
settings-inspection-channel-colours = Kanal renkleri
settings-inspection-channel-colours-help = Renk kanallarının nasıl gösterileceği. Macenta–yeşil ve macenta–sarı–camgöbeği, kırmızı–yeşil renk körlüğünde ayırt edilebilir kalır.
settings-inspection-channel-colours-red-green = Kırmızı, yeşil, mavi
settings-inspection-channel-colours-magenta-green = Macenta, yeşil
settings-inspection-channel-colours-magenta-yellow-cyan = Macenta, sarı, camgöbeği
settings-inspection-colour-vision = Renk görme simülasyonu
settings-inspection-colour-vision-help = Tüm pencereyi bir renk görme eksikliğiyle görüldüğü gibi göster (Machado ve ark. 2009). Dışa aktarımlar hiç etkilenmez.
settings-inspection-colour-vision-off = Kapalı
settings-inspection-colour-vision-protanopia = Protanopi
settings-inspection-colour-vision-deuteranopia = Döteranopi
settings-inspection-colour-vision-tritanopia = Tritanopi
settings-inspection-colour-vision-achromatopsia = Akromatopsi
settings-inspection-auto-range = Yüksek bit derinlikli gri tonlamada otomatik aralık
settings-inspection-auto-range-help = 16 bit gri tonlamalı görüntüleri (TIFF, PNG, FITS) en koyu değerlerinden en parlak değerlerine yay. DICOM görüntüleri kendi pencerelerini korur.

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = Artı imleci açık. Ok tuşları onu taşır, daha büyük adımlar için Shift. Enter tıklar. Esc bitirir.
crosshair-off = Artı imleci kapalı
announce-cursor-nothing = İmlecin altında bir şey yok
