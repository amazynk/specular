# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). Türkçe.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
    [one] Bu düzenlemedeki bir modül daha yeni bir sürüme güncellendi. Sonuç biraz farklı görünebilir.
   *[other] Bu düzenlemedeki { $count } modül daha yeni bir sürüme güncellendi. Sonuç biraz farklı görünebilir.
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
    [one] Bu düzenlemedeki bir modül daha yeni bir Specular sürümü gerektiriyor. Korunuyor ancak gösterilmiyor.
   *[other] Bu düzenlemedeki { $count } modül daha yeni bir Specular sürümü gerektiriyor. Korunuyorlar ancak gösterilmiyorlar.
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = Nokta { $index }/{ $count }: giriş { $input }, çıkış { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = Ok tuşları noktayı taşır. Page Up ve Page Down başka birini seçer. Insert nokta ekler, Delete onu kaldırır.
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = Nötr
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = Ton { $hue }°, güç { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = Ok tuşları rengi taşır. Delete onu sıfırlar.

## Develop's panel.

# machine-drafted, needs native review
develop-title = Geliştirme
# machine-drafted, needs native review
develop-undo = Geri al
# machine-drafted, needs native review
develop-redo = Yinele
# machine-drafted, needs native review
develop-before-after = Önce ve sonra
# machine-drafted, needs native review
develop-close = Geliştirme modundan çık
# machine-drafted, needs native review
develop-history = Geçmiş
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module } açık
develop-card-menu = { $module } seçenekleri
develop-card-changed = Değiştirildi
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }: { $value }
# machine-drafted, needs native review
develop-filmstrip = Film şeridi

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = Pozlama
develop-module-light = Işık
develop-look-profile = Profil
develop-look-standard = Standart
develop-look-film = Film
develop-look-natural = Doğal
develop-module-grading = Renk derecelendirme
develop-add-tool = Araç ekle
# machine-drafted, needs native review
develop-module-white-balance = Beyaz dengesi
# machine-drafted, needs native review
develop-module-tone = Ton
# machine-drafted, needs native review
develop-module-contrast = Kontrast ve aralık
# machine-drafted, needs native review
develop-module-colour = Renk
# machine-drafted, needs native review
develop-module-curve = Eğri
# machine-drafted, needs native review
develop-module-crop = Kırp ve düzelt
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = Diğer modül
# machine-drafted, needs native review
develop-exposure = Pozlama
# machine-drafted, needs native review
develop-black-level = Siyah seviyesi
# machine-drafted, needs native review
develop-temperature = Sıcaklık
# machine-drafted, needs native review
develop-tint = Renk tonu
# machine-drafted, needs native review
develop-highlights = Vurgular
# machine-drafted, needs native review
develop-shadows = Gölgeler
# machine-drafted, needs native review
develop-contrast = Kontrast
# machine-drafted, needs native review
develop-whites = Beyazlar
# machine-drafted, needs native review
develop-blacks = Siyahlar
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = HDR sınırı
# machine-drafted, needs native review
develop-hdr-preview = Göster
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = Canlılık
# machine-drafted, needs native review
develop-saturation = Doygunluk
# machine-drafted, needs native review
develop-colour-wheels = Renk çarkları
# machine-drafted, needs native review
develop-curve = Ton eğrisi
# machine-drafted, needs native review
develop-module-local-contrast = Yerel kontrast
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = Otomatik pozlama
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = Yumuşatma
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = Doku
# machine-drafted, needs native review
develop-clarity = Netlik
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = Siyah noktası
# machine-drafted, needs native review
develop-levels-mid = Orta nokta
# machine-drafted, needs native review
develop-levels-white = Beyaz noktası
# machine-drafted, needs native review
develop-module-detail = Ayrıntı
# machine-drafted, needs native review
develop-module-denoise = Gürültü azaltma
# machine-drafted, needs native review
develop-module-sharpen = Keskinleştirme
# machine-drafted, needs native review
develop-noise-luminance = Parlaklık gürültüsü
# machine-drafted, needs native review
develop-noise-detail = Gürültü ayrıntısı
# machine-drafted, needs native review
develop-noise-colour = Renk gürültüsü
# machine-drafted, needs native review
develop-sharpen-amount = Keskinleştirme miktarı
# machine-drafted, needs native review
develop-sharpen-radius = Keskinleştirme yarıçapı
# machine-drafted, needs native review
develop-sharpen-detail = Keskinleştirme ayrıntısı
# machine-drafted, needs native review
develop-sharpen-masking = Maskeleme
# machine-drafted, needs native review
develop-capture-amount = Çekim keskinleştirme
# machine-drafted, needs native review
develop-capture-radius = Çekim yarıçapı
# machine-drafted, needs native review
develop-straighten = Düzelt
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = Kırp
# machine-drafted, needs native review
develop-crop-turn = Döndür
# machine-drafted, needs native review
develop-crop-flip = Çevir
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = Lens düzeltmeleri
develop-module-optics = Optik
# machine-drafted, needs native review
develop-lens-distortion = Bozulma
# machine-drafted, needs native review
develop-lens-vignette = Vinyet
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = Kırmızı saçak
# machine-drafted, needs native review
develop-lens-tca-blue = Mavi saçak
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = Lens profili
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = Otomatik
# machine-drafted, needs native review
develop-lensfun-off = Yok
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = Lens profili yok.
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = { $lens } için Lensfun profili yok.
# machine-drafted, needs native review
develop-lensfun-no-profile = Bu lens için Lensfun profili yok.
# machine-drafted, needs native review
develop-lensfun-no-name = Dosya lensin adını vermiyor. Yukarıdan seçin.
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = { $count } lens eşleşiyor. Yukarıdan birini seçin.

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = Kırp. Kenarları sürükleyin veya düz olması gereken bir çizgi boyunca Ctrl+sürükleyin. Enter korur, Esc geri döner.
# machine-drafted, needs native review
develop-crop-kept = Kırpma korundu
# machine-drafted, needs native review
develop-crop-cancelled = Kırpma geri alındı
# machine-drafted, needs native review
develop-crop-reset = Kırpma ve düzeltme sıfırlandı
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = { $angle }° düzeltildi
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = En boy oranı { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = serbest
# machine-drafted, needs native review
develop-crop-aspect-original = özgün
# machine-drafted, needs native review
develop-crop-turned-right = Sağa döndürüldü
# machine-drafted, needs native review
develop-crop-turned-left = Sola döndürüldü
# machine-drafted, needs native review
develop-crop-flipped-horizontal = Soldan sağa çevrildi
# machine-drafted, needs native review
develop-crop-flipped-vertical = Yukarıdan aşağıya çevrildi
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = En boy oranı { $aspect } · Kılavuz: { $guide }
# machine-drafted, needs native review
develop-crop-keys = A en boy oranı · X değiştir · O kılavuz · [ ] döndür · H V çevir · Enter koru · Esc geri
# machine-drafted, needs native review
develop-crop-guide-thirds = üçler kuralı
# machine-drafted, needs native review
develop-crop-guide-golden = altın oran
# machine-drafted, needs native review
develop-crop-guide-diagonal = köşegenler
# machine-drafted, needs native review
develop-crop-guide-grid = ızgara

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = Perspektif
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = Dikleştirme
# machine-drafted, needs native review
develop-persp-mode-off = Kapalı
# machine-drafted, needs native review
develop-persp-mode-auto = Otomatik
# machine-drafted, needs native review
develop-persp-mode-level = Seviye
# machine-drafted, needs native review
develop-persp-mode-vertical = Dikey
# machine-drafted, needs native review
develop-persp-mode-full = Tam
# machine-drafted, needs native review
develop-persp-mode-guided = Kılavuzlu
# machine-drafted, needs native review
develop-persp-vertical = Dikey
# machine-drafted, needs native review
develop-persp-horizontal = Yatay
# machine-drafted, needs native review
develop-persp-rotate = Döndür
# machine-drafted, needs native review
develop-persp-aspect = En boy
# machine-drafted, needs native review
develop-persp-scale = Ölçek
# machine-drafted, needs native review
develop-persp-offset-x = Yana
# machine-drafted, needs native review
develop-persp-offset-y = Yukarı veya aşağı
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = Boş kenarlardan kaçın
# machine-drafted, needs native review
develop-persp-guides = Kılavuzlar
# machine-drafted, needs native review
develop-persp-focal = Odak uzaklığı
# machine-drafted, needs native review
develop-persp-draw-guides = Kılavuz çiz
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = Kılavuz çiz ({ $n } / { $max })
# machine-drafted, needs native review
develop-persp-reset = Perspektifi sıfırla
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = Dikleştirme: { $mode }
# machine-drafted, needs native review
develop-persp-analysing = Çizgiler aranıyor…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = Perspektif sıfırlansın mı? { $n } kılavuzunuz kaldırılır. Geri al onları geri getirir.
# machine-drafted, needs native review
develop-persp-reset-ok = Sıfırla
# machine-drafted, needs native review
develop-persp-reset-done = Perspektif sıfırlandı
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = Kılavuz { $n } kaldırılsın mı? Geri al onu geri getirir.
# machine-drafted, needs native review
develop-persp-guide-remove-ok = Kaldır
# machine-drafted, needs native review
develop-persp-guide-removed = Kılavuz kaldırıldı
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = Dikey { $vertical }, yatay { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = Dikleştirme başarısız: { $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }: eğim { $vertical }°, dönüş { $horizontal }°, döndürme { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } için ayırt edilebilen en az iki düz çizgi gerekir. Başka bir mod deneyin veya kılavuz çizin.
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = En fazla { $n } kılavuz. Birini taşımak için ucunu sürükleyin.
# machine-drafted, needs native review
develop-persp-guides-on = Kılavuzlar. Dikey veya yatay olması gereken bir çizgi boyunca sürükleyin.
# machine-drafted, needs native review
develop-persp-guides-off = Kırp
# machine-drafted, needs native review
develop-persp-guide-moved = Kılavuz taşındı
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = Kılavuz { $n } eklendi
# machine-drafted, needs native review
develop-persp-guides-keys = Sürükle: ekle · Ucu sürükle: taşı · Delete kaldırır · Shift+R kırp
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = Kılavuzlar: { $n } / { $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = Gölgeler
# machine-drafted, needs native review
develop-wheel-midtones = Orta tonlar
# machine-drafted, needs native review
develop-wheel-highlights = Vurgular
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = Gölgelerin parlaklığı
# machine-drafted, needs native review
develop-luminance-midtones = Orta tonların parlaklığı
# machine-drafted, needs native review
develop-luminance-highlights = Vurguların parlaklığı
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = Genel
# machine-drafted, needs native review
develop-luminance-global = Genel parlaklık
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = Renk yoğunluğu
# machine-drafted, needs native review
develop-brilliance = Renk parlaklığı

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = Renk karıştırıcı
# machine-drafted, needs native review
develop-colour-red = Kırmızı
# machine-drafted, needs native review
develop-colour-orange = Turuncu
# machine-drafted, needs native review
develop-colour-yellow = Sarı
# machine-drafted, needs native review
develop-colour-green = Yeşil
# machine-drafted, needs native review
develop-colour-aqua = Su yeşili
# machine-drafted, needs native review
develop-colour-blue = Mavi
# machine-drafted, needs native review
develop-colour-purple = Mor
# machine-drafted, needs native review
develop-colour-magenta = Macenta
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = { $colour } tonu
# machine-drafted, needs native review
develop-mixer-saturation = { $colour } doygunluğu
# machine-drafted, needs native review
develop-mixer-brightness = { $colour } parlaklığı

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = Nokta rengi
# machine-drafted, needs native review
develop-point-pick = Renk seç
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = Başka renk seç
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = Değiştirilecek renge tıklayın. Esc iptal eder.
# machine-drafted, needs native review
develop-point-picked = Renk seçildi
# machine-drafted, needs native review
develop-point-cancelled = Seçim iptal edildi
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = Aralık
# machine-drafted, needs native review
develop-point-hue-range = Ton aralığı
# machine-drafted, needs native review
develop-point-saturation-range = Doygunluk aralığı
# machine-drafted, needs native review
develop-point-lightness-range = Açıklık aralığı
# machine-drafted, needs native review
develop-point-hue = Ton kaydırma
# machine-drafted, needs native review
develop-point-saturation = Doygunluk kaydırma
# machine-drafted, needs native review
develop-point-lightness = Açıklık kaydırma

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = Siyah beyaz
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = { $colour } parlaklığı

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = LUT dosyası seç…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = Dosya: { $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = { $name } okunamıyor
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = LUT dosyası seç
# machine-drafted, needs native review
develop-lut-kind = LUT dosyaları
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = Hedef renk uzayı
# machine-drafted, needs native review
develop-lut-space-linear = Doğrusal (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES doğrusal (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES log (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = Fotoğraf (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = Video (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = Yöntem
# machine-drafted, needs native review
develop-lut-tetrahedral = Hassas
# machine-drafted, needs native review
develop-lut-trilinear = Basit
# machine-drafted, needs native review
develop-lut-strength = Güç

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = Özgün
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module } açık
# machine-drafted, needs native review
develop-history-off = { $module } kapalı
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = Özgün hâline dönüldü
# machine-drafted, needs native review
develop-at-step = Geri dönüldü: { $step }
# machine-drafted, needs native review
develop-undone = Geri alındı: { $step }
# machine-drafted, needs native review
develop-redone = Yinelendi: { $step }
# machine-drafted, needs native review
develop-entered = Geliştirme. Düzenlemeler dosyayı asla değiştirmez.
# machine-drafted, needs native review
develop-left = Geliştirme modundan çıkıldı
# machine-drafted, needs native review
develop-split-on = Önce ve sonra yan yana
# machine-drafted, needs native review
develop-split-off = Yalnızca düzenleme
# machine-drafted, needs native review
develop-showing-before = Özgün gösteriliyor
# machine-drafted, needs native review
develop-showing-after = Düzenleme gösteriliyor
# machine-drafted, needs native review
# Labels over the image.
develop-before = Önce
# machine-drafted, needs native review
develop-after = Sonra
# machine-drafted, needs native review
develop-working = Geliştiriliyor…
# machine-drafted, needs native review
develop-preview-failed = Bu dosya için düzenleme gösterilemiyor

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = Geliştirme
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = Hızlı düzenleme: basılı tutun, sonra kaydırın
# machine-drafted, needs native review
palette-develop = Geliştirme
# machine-drafted, needs native review
palette-develop-help = Bu fotoğrafı dosyayı değiştirmeden düzenler. Çıkmak için yeniden çalıştırın.
# machine-drafted, needs native review
palette-develop-before-after = Önce ve sonra
# machine-drafted, needs native review
palette-develop-before-after-help = Özgünü ya da yeniden düzenlemeyi gösterir.
# machine-drafted, needs native review
palette-develop-split = Önce ve sonra bölünmüş
# machine-drafted, needs native review
palette-develop-split-help = Özgünü ve düzenlemeyi yan yana gösterir.
# machine-drafted, needs native review
palette-develop-panel = Geliştirme paneline git
# machine-drafted, needs native review
palette-develop-panel-help = Klavyeyi panele taşır. Esc geri döner.
# machine-drafted, needs native review
palette-develop-speed-exposure = Pozlamayı hızlı düzenle
# machine-drafted, needs native review
palette-develop-speed-exposure-help = Pozlamayı değiştirmek için basılı tutun, sonra kaydırın ya da ok tuşlarına basın.
# machine-drafted, needs native review
palette-develop-speed-contrast = Kontrastı hızlı düzenle
# machine-drafted, needs native review
palette-develop-speed-contrast-help = Kontrastı değiştirmek için basılı tutun, sonra kaydırın ya da ok tuşlarına basın.
# machine-drafted, needs native review
palette-develop-speed-highlights = Vurguları hızlı düzenle
# machine-drafted, needs native review
palette-develop-speed-highlights-help = Vurguları değiştirmek için basılı tutun, sonra kaydırın ya da ok tuşlarına basın.
# machine-drafted, needs native review
palette-develop-speed-shadows = Gölgeleri hızlı düzenle
# machine-drafted, needs native review
palette-develop-speed-shadows-help = Gölgeleri değiştirmek için basılı tutun, sonra kaydırın ya da ok tuşlarına basın.
# machine-drafted, needs native review
palette-develop-speed-whites = Beyazları hızlı düzenle
# machine-drafted, needs native review
palette-develop-speed-whites-help = Beyazları değiştirmek için basılı tutun, sonra kaydırın ya da ok tuşlarına basın.
# machine-drafted, needs native review
palette-develop-speed-blacks = Siyahları hızlı düzenle
# machine-drafted, needs native review
palette-develop-speed-blacks-help = Siyahları değiştirmek için basılı tutun, sonra kaydırın ya da ok tuşlarına basın.
# machine-drafted, needs native review
palette-develop-speed-temperature = Sıcaklığı hızlı düzenle
# machine-drafted, needs native review
palette-develop-speed-temperature-help = Beyaz dengesinin sıcaklığını değiştirmek için basılı tutun, sonra kaydırın ya da ok tuşlarına basın.
# machine-drafted, needs native review
palette-develop-speed-vibrance = Canlılığı hızlı düzenle
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = Canlılığı değiştirmek için basılı tutun, sonra kaydırın ya da ok tuşlarına basın.
# machine-drafted, needs native review
palette-develop-crop = Kırp ve düzelt
# machine-drafted, needs native review
palette-develop-crop-help = Fotoğrafı kırpın, düzeltin, döndürün veya çevirin. Enter korur, Esc geri döner.
# machine-drafted, needs native review
disabled-not-photo = Yalnızca fotoğraflar için
# machine-drafted, needs native review
disabled-develop-off = Geliştirme, Tıp ve Bilim profillerinde kapalıdır
# machine-drafted, needs native review
develop-module-corrections = Düzeltmeler
# machine-drafted, needs native review
develop-module-raw = Raw
# machine-drafted, needs native review
develop-off = Kapalı
# machine-drafted, needs native review
develop-on = Açık
# machine-drafted, needs native review
develop-highlight-method = Patlamış parlak alanlar
# machine-drafted, needs native review
develop-highlight-opposed = Rengi yeniden kur
# machine-drafted, needs native review
develop-highlight-rebuild = Büyük alanları yeniden kur
# machine-drafted, needs native review
develop-highlight-clip = Beyaza kırp
# machine-drafted, needs native review
develop-highlight-threshold = Kırpılma düzeyi
# machine-drafted, needs native review
develop-module-hot-pixels = Sıcak pikseller
# machine-drafted, needs native review
develop-hot-threshold = Sıcak piksel eşiği
# machine-drafted, needs native review
develop-module-ca = Renk sapması
# machine-drafted, needs native review
develop-ca-mode = Saçaklar
# machine-drafted, needs native review
develop-ca-auto = Otomatik
# machine-drafted, needs native review
develop-ca-manual = Elle
# machine-drafted, needs native review
develop-ca-red = Kırmızı kayması
# machine-drafted, needs native review
develop-ca-blue = Mavi kayması
# machine-drafted, needs native review
develop-module-defringe = Renk saçağı giderme
# machine-drafted, needs native review
develop-defringe = Renk saçağı giderme
# machine-drafted, needs native review
develop-defringe-purple = Mor ton
# machine-drafted, needs native review
develop-defringe-green = Yeşil ton
# machine-drafted, needs native review
develop-defringe-range = Ton aralığı
# machine-drafted, needs native review
develop-defringe-edge = Kenar kontrastı
# machine-drafted, needs native review
develop-module-flat-field = Kalibrasyon kareleri
# machine-drafted, needs native review
develop-calibration = Kalibrasyon
# machine-drafted, needs native review
develop-calibration-auto = Klasörden
# machine-drafted, needs native review
develop-calibration-chosen = Seçilen dosyalar
# machine-drafted, needs native review
develop-flat-choose = Flat kare seç…
# machine-drafted, needs native review
develop-dark-choose = Dark kare seç…
# machine-drafted, needs native review
develop-flat-file = Flat kare: { $name }
# machine-drafted, needs native review
develop-dark-file = Dark kare: { $name }
# machine-drafted, needs native review
develop-flat-use = Flat kare
# machine-drafted, needs native review
develop-flat-use-full = Vinyet ve toz
# machine-drafted, needs native review
develop-flat-use-vignetting = Yalnızca vinyet
# machine-drafted, needs native review
develop-flat-dialog = Flat kare seç
# machine-drafted, needs native review
develop-dark-dialog = Dark kare seç
# machine-drafted, needs native review
develop-raw-kind = Raw dosyaları
# machine-drafted, needs native review
develop-flat-frame = Flat kare
# machine-drafted, needs native review
develop-dark-frame = Dark kare

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = Maske ekle
# machine-drafted, needs native review
develop-mask-kind-ellipse = Elips
# machine-drafted, needs native review
develop-mask-kind-rectangle = Dikdörtgen
# machine-drafted, needs native review
develop-mask-kind-polygon = Çokgen
# machine-drafted, needs native review
develop-mask-kind-linear = Doğrusal geçiş
# machine-drafted, needs native review
develop-mask-kind-radial = Dairesel geçiş
# machine-drafted, needs native review
develop-mask-kind-brush = Fırça
# machine-drafted, needs native review
develop-mask-kind-luminance = Parlaklık aralığı
# machine-drafted, needs native review
develop-mask-kind-hue = Ton aralığı
# machine-drafted, needs native review
develop-mask-kind-chroma = Doygunluk aralığı
# machine-drafted, needs native review
develop-mask-kind-colour = Renk aralığı
# machine-drafted, needs native review
develop-mask-kind-other = Diğer maske
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = Karıştırma
# machine-drafted, needs native review
develop-blend-normal = Normal
# machine-drafted, needs native review
develop-blend-multiply = Çarp
# machine-drafted, needs native review
develop-blend-screen = Ekran
# machine-drafted, needs native review
develop-blend-overlay = Kaplama
# machine-drafted, needs native review
develop-blend-soft-light = Yumuşak ışık
# machine-drafted, needs native review
develop-blend-lightness = Yalnız açıklık
# machine-drafted, needs native review
develop-blend-colour = Yalnız renk
# machine-drafted, needs native review
develop-mask-amount = Maske gücü
# machine-drafted, needs native review
develop-mask-invert = Maskeyi ters çevir
# machine-drafted, needs native review
develop-mask-off = Kapalı
# machine-drafted, needs native review
develop-mask-on = Açık
# machine-drafted, needs native review
develop-mask-refine = Kenarları iyileştir
# machine-drafted, needs native review
develop-mask-show = Maskeyi göster
# machine-drafted, needs native review
develop-mask-show-red = Kırmızı
# machine-drafted, needs native review
develop-mask-show-grey = Gri
# machine-drafted, needs native review
develop-mask-remove = Maskeyi kaldır
# machine-drafted, needs native review
develop-mask-combine-add = Ekle
# machine-drafted, needs native review
develop-mask-combine-subtract = Çıkar
# machine-drafted, needs native review
develop-mask-combine-intersect = Kesişim
# machine-drafted, needs native review
develop-mask-row-opacity = Opaklık
# machine-drafted, needs native review
develop-mask-row-invert = Ters çevir
# machine-drafted, needs native review
develop-mask-feather = Yumuşak kenar
# machine-drafted, needs native review
develop-mask-roundness = Yuvarlaklık
# machine-drafted, needs native review
develop-mask-brush-size = Fırça boyutu
# machine-drafted, needs native review
develop-mask-brush-feather = Fırça yumuşaklığı
# machine-drafted, needs native review
develop-mask-brush-flow = Akış
# machine-drafted, needs native review
develop-mask-brush-mode = Fırça
# machine-drafted, needs native review
develop-mask-brush-paint = Boya
# machine-drafted, needs native review
develop-mask-brush-erase = Sil
# machine-drafted, needs native review
develop-mask-range-from = Başlangıç
# machine-drafted, needs native review
develop-mask-range-full-from = Tam başlangıç
# machine-drafted, needs native review
develop-mask-range-full-to = Tam bitiş
# machine-drafted, needs native review
develop-mask-range-to = Bitiş
# machine-drafted, needs native review
develop-mask-range-test = Sına
# machine-drafted, needs native review
develop-mask-range-input = Giriş
# machine-drafted, needs native review
develop-mask-range-output = Çıkış
# machine-drafted, needs native review
develop-mask-colour-pick = Renk seç
# machine-drafted, needs native review
develop-mask-colour-pick-more = Bir renk daha seç
# machine-drafted, needs native review
develop-mask-colour-range = Aralık
# machine-drafted, needs native review
develop-mask-colour-falloff = Azalma
# machine-drafted, needs native review
develop-mask-edit = Resim üzerinde düzenle
# machine-drafted, needs native review
develop-mask-row-remove = { $name } kaldır
# machine-drafted, needs native review
develop-mask-remove-confirm = { $module } maskesi kaldırılsın mı? Şekilleri gider. Diskteki fotoğraf kalır.
# machine-drafted, needs native review
develop-mask-row-remove-confirm = { $name }, { $module } modülünden kaldırılsın mı? Diğer maskeler ve diskteki fotoğraf kalır.
# machine-drafted, needs native review
develop-mask-remove-ok = Kaldır
# machine-drafted, needs native review
develop-mask-removed = Maske kaldırıldı.
# machine-drafted, needs native review
develop-mask-added = { $name } eklendi.
# machine-drafted, needs native review
develop-mask-full = Bu maske alabileceği kadar parçaya sahip.
# machine-drafted, needs native review
develop-mask-editing = { $name } düzenleniyor. Tutamaçları sürükleyin ya da Tab ve okları kullanın.
# machine-drafted, needs native review
develop-mask-painting = { $name } boyanıyor. Resim üzerinde sürükleyin.
# machine-drafted, needs native review
develop-mask-picking = Renk seçmek için resme tıklayın.
# machine-drafted, needs native review
develop-mask-picked = Renk seçildi.
# machine-drafted, needs native review
develop-mask-done = Maske düzenleme bitti.
# machine-drafted, needs native review
develop-mask-handles = Maske tutamaçları
# machine-drafted, needs native review
develop-handle-centre = Merkez
# machine-drafted, needs native review
develop-handle-width = Genişlik
# machine-drafted, needs native review
develop-handle-height = Yükseklik
# machine-drafted, needs native review
develop-handle-rotate = Döndür
# machine-drafted, needs native review
develop-handle-feather = Yumuşak kenar
# machine-drafted, needs native review
develop-handle-inner = İç kenar
# machine-drafted, needs native review
develop-handle-start = Başlangıç
# machine-drafted, needs native review
develop-handle-end = Bitiş
# machine-drafted, needs native review
develop-handle-point = Nokta { $n }
# machine-drafted, needs native review
develop-instance = Katman
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = Katman { $n }.
# machine-drafted, needs native review
develop-instance-new = Yeni katman
# machine-drafted, needs native review
develop-instance-duplicate = Katmanı çoğalt
# machine-drafted, needs native review
develop-instance-delete = Katmanı sil
# machine-drafted, needs native review
develop-instance-earlier = Öne al
# machine-drafted, needs native review
develop-instance-later = Arkaya al
# machine-drafted, needs native review
develop-instance-delete-confirm = { $module } modülünün { $n }. katmanı silinsin mi? Ayarları ve maskeleri gider. Diskteki fotoğraf kalır.
# machine-drafted, needs native review
develop-instance-delete-ok = Sil
# machine-drafted, needs native review
develop-instance-deleted = Katman silindi.
# machine-drafted, needs native review
develop-history-mask = Maske: { $module }
# machine-drafted, needs native review
develop-history-mask-shapes = Maske şekilleri: { $module }
# machine-drafted, needs native review
develop-history-instance-new = Yeni: { $module }
# machine-drafted, needs native review
develop-history-instance-delete = Silindi: { $module }
# machine-drafted, needs native review
develop-history-instance-move = Sıra: { $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = Diğer programlar
# machine-drafted, needs native review
develop-darktable-edited = darktable’da düzenlendi:
# machine-drafted, needs native review
develop-darktable-open = darktable’da aç
# machine-drafted, needs native review
develop-darktable-opening = darktable’da açılıyor
# machine-drafted, needs native review
develop-darktable-failed = darktable başlamadı: { $reason }
# machine-drafted, needs native review
develop-lightroom-import = Lightroom’dan al
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
    [one] 1 Lightroom ayarı içe aktarıldı. Sonuç yaklaşıktır.
   *[other] { $count } Lightroom ayarı içe aktarıldı. Sonuç yaklaşıktır.
}
# machine-drafted, needs native review
develop-lightroom-nothing = İçe aktarılacak bir şey yok: bu ayarlar burada bir şey değiştirmiyor.
# machine-drafted, needs native review
develop-history-lightroom = Lightroom’dan içe aktarıldı (yaklaşık)
# machine-drafted, needs native review
develop-conflict-title = { $file } düzenlemesi Specular dışında değişti
# machine-drafted, needs native review
develop-conflict-file = Yan dosya: { $time } kaydedildi
# machine-drafted, needs native review
develop-conflict-library = Kitaplık: { $time } kaydedildi
# machine-drafted, needs native review
develop-conflict-writer = Yan dosyayı { $writer } yazdı.
# machine-drafted, needs native review
develop-conflict-detail = Birini tutun. Diğeri değiştirilir.
# machine-drafted, needs native review
develop-conflict-use-file = Dosyadakini kullan
# machine-drafted, needs native review
develop-conflict-keep-library = Kitaplıktakini tut
# machine-drafted, needs native review
develop-conflict-used-file = Yan dosyadaki düzenleme kullanılıyor
# machine-drafted, needs native review
develop-conflict-kept-library = Kitaplıktaki düzenleme tutuluyor
# machine-drafted, needs native review
toast-develop-edit-newer-process = Bu düzenleme daha yeni bir Specular ile yapıldı. Burada farklı görünebilir.
# machine-drafted, needs native review
info-darktable-edited = darktable’da düzenlendi
# machine-drafted, needs native review
info-darktable-no-modules = Açık modül yok
# machine-drafted, needs native review
info-darktable-group = Dosya
# machine-drafted, needs native review
toast-develop-edit-partial = Bu düzenleme daha yeni bir Specular ile yapıldı. Bu sürümün gösteremediği kısımlar atlandı. Buradaki değişiklikler kaydedilmez.
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = Fotoğrafın yanındaki düzenleme dosyası okunamıyor. Olduğu gibi bırakılır. Buradaki değişiklikler kaydedilmez.
# machine-drafted, needs native review
develop-demosaic = Raw ayrıntısı
develop-demosaic-directional = Standart
develop-demosaic-dual = Düz alanları yumuşat
develop-demosaic-fast = Hızlı
develop-dual-threshold = Düz alan kontrastı
