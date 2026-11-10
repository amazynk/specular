# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. Türkçe. machine-drafted, needs native review

palette-tether = Kablolu çekim…
palette-tether-capture = Fotoğraf çek (kablolu)

tether-title = Kablolu çekim
tether-close = Kablolu çekimi kapat
tether-default-folder = Kablolu çekim

## Cameras
tether-none = Kamera bulunamadı.
tether-none-hint = Kamerayı USB kablosuyla bağlayıp açın. USB modu varsa PTP ya da bilgisayar bağlantısını seçin.
tether-look-again = Yeniden ara
tether-connect = Bağlan
tether-next-camera = Sonraki kamera
tether-connecting = Bağlanıyor…
tether-connected = { $camera } bağlandı
tether-disconnect = Bağlantıyı kes
tether-try-again = Yeniden dene
tether-gone = Kameranın bağlantısı kesildi.
tether-not-connected = Bağlı kamera yok. Önce kablolu çekimi açın.

## Why a camera can't be opened
tether-permission = Bu kullanıcı kameranın USB aygıtını açamaz { $node }.
tether-permission-rule = Bir udev kuralı buna izin verir. Yönetici olarak bu satırı { $file } dosyasına ekleyin:
tether-permission-reload = Sonra "sudo udevadm control --reload" komutunu çalıştırıp kamerayı yeniden takın.
tether-copy-rule = Kuralı kopyala
tether-busy-other = Kamerayı başka bir program kullanıyor.
tether-busy-named = Kamerayı { $programs } kullanıyor.
tether-busy-hint = O programı kapatın ya da kamerayı dosya yöneticisinde ayırın, sonra yeniden deneyin.
tether-driver = Bu kameranın sistem sürücüsü başka programların kullanmasına izin vermiyor.

## The camera
tether-battery = Pil { PERCENT($percent) }
tether-card = { $card }: { $free } boş
tether-card-shots = { $card }: { $free } boş, { $shots } çekim
tether-no-card = Bellek kartı yok.
tether-no-settings = Kamera pozlama ayarlarını bildirmiyor.
tether-no-remote = Bu kamera buradan tetiklenemez. Kendi deklanşörüyle çekilen fotoğraflar indirilir.
tether-capture = Çek
tether-step-down = { $setting }: azalt ({ $value })
tether-step-up = { $setting }: artır ({ $value })

## Exposure settings
tether-prop-mode = Mod
tether-prop-shutter = Enstantane
tether-prop-aperture = Diyafram
tether-prop-iso = ISO
tether-prop-bias = Pozlama telafisi
tether-prop-white-balance = Beyaz dengesi
tether-auto = Otomatik
tether-bulb = Bulb (B) modu
tether-camera-value = Kamera ayarı { $value }
tether-wb-manual = Elle
tether-wb-one-push = Tek seferlik ölçüm
tether-wb-daylight = Gün ışığı
tether-wb-fluorescent = Floresan
tether-wb-tungsten = Tungsten ışığı
tether-wb-flash = Flaş
tether-mode-manual = Elle
tether-mode-program = Program modu
tether-mode-aperture = Diyafram önceliği
tether-mode-shutter = Enstantane önceliği
tether-mode-creative = Yaratıcı
tether-mode-action = Spor
tether-mode-portrait = Portre

## Where shots go
tether-folder = Klasör
tether-names = Adlar
tether-names-camera = Kameranın verdiği adlar
tether-arrival = Develop stili
tether-choose-folder = Klasör…
tether-choose-style = Stil…
tether-downloading = İndiriliyor… { PERCENT($percent) }
tether-saved = { $name } kaydedildi
tether-saved-count = { $count ->
    [one] { $count } çekim kaydedildi
   *[other] { $count } çekim kaydedildi
}

## Failures
tether-busy = Kamera meşgul. Yeniden deneyin.
tether-card-full = Bellek kartı dolu.
tether-set-refused = Kamera bu ayarı kabul etmedi. Mod kadranını kontrol edin.
tether-disk = Çekim kaydedilemedi: { $error }
tether-error = Kamera hatası: { $error }

## Settings › Cull mode
settings-cull-tether-folder = Kablolu çekim klasörü
settings-cull-tether-folder-help = Kabloyla bağlı bir kameranın çekimlerinin kaydedildiği yer.
settings-cull-tether-folder-unset = Resimler › Kablolu çekim
settings-cull-tether-rename = Kablolu çekim dosya adları
settings-cull-tether-rename-help = İçe Aktar'daki Yeniden Adlandır ile aynı etiketlerle yeni adlar. Boş bırakılırsa kameranın adları korunur.
