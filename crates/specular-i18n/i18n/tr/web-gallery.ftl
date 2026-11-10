# Specular — Müşteri galerileri: fotoğraflardan statik bir HTML klasörü, ve müşterinin seçimlerini geri okuma.

# machine-drafted, needs native review

palette-client-gallery = Müşteri galerisi…
palette-client-gallery-help = Fotoğrafları bir müşteri için web sayfası klasörü olarak kaydet; müşteri seçimlerini geri gönderebilir.
palette-import-client-picks = Müşteri seçimlerini içe aktar…
palette-import-client-picks-help = Bir müşterinin galerisinde seçtiği fotoğrafları işaretle.

webgallery-heading = Müşteri galerisi
webgallery-subtitle = { $count ->
    [one] { $count } fotoğraf
   *[other] { $count } fotoğraf
}
webgallery-title-label = Başlık
webgallery-title-default = Prova baskılar
webgallery-watermark-label = Filigran metni (boş: yok)
webgallery-source = Fotoğraflar
webgallery-source-current = Bu fotoğraf
webgallery-source-selection = Seçim
webgallery-source-album = “{ $name }” albümü
webgallery-source-search = Arama sonuçları
webgallery-source-folder = Bu klasör
webgallery-source-picks = Seçilenler
webgallery-source-value = { $what } ({ $count })
webgallery-theme = Tema
webgallery-theme-light = Açık
webgallery-theme-dark = Koyu
webgallery-theme-minimal = Sade
webgallery-size = Uzun kenar
webgallery-size-value = { $px } px
webgallery-format = Biçim
webgallery-captions = Her fotoğrafın altında
webgallery-captions-off = Hiçbir şey
webgallery-captions-name = Dosya adı
webgallery-captions-title = Başlık
webgallery-captions-all = Dosya adı, başlık ve açıklama
webgallery-exif = Kamera ve pozlamayı göster
webgallery-location = Fotoğraflarda konumu (GPS) koru
webgallery-note-private = Statik bir galeri parolayla korunamaz.
webgallery-note-share = Klasöre ya da adresine sahip olan herkes her fotoğrafı görür. Bağlantıyı yalnızca müşterinizle paylaşın ya da web barındırıcınızın erişim denetimini kullanın.
webgallery-note-picks = Müşteriler kalple seçer. Seçimler, listeyi size gönderene kadar tarayıcılarında kalır.
webgallery-write = Galeri oluştur…
webgallery-busy = Yazılıyor: { $done } / { $total }…
webgallery-stop = Durdur
webgallery-done = { $count ->
    [one] Galeri oluşturuldu: { $folder } içinde { $count } fotoğraf
   *[other] Galeri oluşturuldu: { $folder } içinde { $count } fotoğraf
}
webgallery-done-failed = { $failed ->
    [one] { $failed } fotoğraf yazılamadı.
   *[other] { $failed } fotoğraf yazılamadı.
}
webgallery-stopped = Galeri durduruldu. Hiçbir şey tutulmadı.
webgallery-failed = Galeri oluşturulmadı: { $reason }
webgallery-nothing = Hiçbir fotoğraf yazılamadı.

webgallery-page-count = { $count ->
    [one] { $count } fotoğraf
   *[other] { $count } fotoğraf
}
webgallery-page-hint = İstediğiniz fotoğraflardaki kalbe dokunun. Seçimleriniz, gönderene kadar bu tarayıcıda kalır.
webgallery-page-picked = Seçilen: {"{"}n{"}"}
webgallery-page-pick = Seç
webgallery-page-copy = Seçimlerimi kopyala
webgallery-page-copied = Kopyalandı. Listeyi bir iletiye yapıştırın.
webgallery-page-copy-by-hand = Seçili listeyi kopyalayıp bir iletiye yapıştırın.
webgallery-page-download-txt = picks.txt indir
webgallery-page-download-json = picks.json indir
webgallery-page-clear = Seçimleri temizle
webgallery-page-clear-ask = Bu tarayıcıdaki tüm seçimleriniz temizlensin mi?
webgallery-page-no-storage = Bu tarayıcı seçimlerinizi saklamayacak. Sayfayı kapatmadan önce gönderin.
webgallery-page-noscript = Seçim için JavaScript gerekir. Fotoğraflar yine de açılır.
webgallery-page-previous = Önceki
webgallery-page-next = Sonraki
webgallery-page-close = Kapat
webgallery-page-position = {"{"}i{"}"} / {"{"}n{"}"}
webgallery-page-viewer = Fotoğraf görüntüleyici
webgallery-page-picks = Seçimleriniz

clientpicks-heading = Müşteri seçimlerini içe aktar
clientpicks-subtitle = { $count ->
    [one] Gösterilen { $count } fotoğrafla eşleştirildi
   *[other] Gösterilen { $count } fotoğrafla eşleştirildi
}
clientpicks-paste-label = Adlar (müşterinin listesini yapıştırın)
clientpicks-choose = Seçim dosyası seç…
clientpicks-from-file = Kaynak: { $name }
clientpicks-apply-as = Şöyle işaretle
clientpicks-as-pick = Seçilmiş olarak
clientpicks-as-stars = { $stars ->
    [one] { $stars } yıldız
   *[other] { $stars } yıldız
}
clientpicks-as-label = { $label } etiketi
clientpicks-empty = Müşterinin gönderdiği listeyi yapıştırın ya da picks.txt veya picks.json dosyasını seçin.
clientpicks-matched = { $total } addan { $matched } tanesi buradaki fotoğraflarla eşleşiyor.
clientpicks-missing = Bulunamadı: { $names }
clientpicks-ambiguous = Birden çok klasörde, dokunulmadı: { $names }
clientpicks-more = { $names } ve { $more } tane daha
clientpicks-apply = Uygula
clientpicks-applied = { $count ->
    [one] { $count } fotoğraf işaretlendi.
   *[other] { $count } fotoğraf işaretlendi.
}
clientpicks-unchanged = Bu fotoğraflar zaten işaretliydi.
clientpicks-read-failed = { $name } okunamıyor: { $reason }
