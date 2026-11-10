# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = Meta veriyi düzenle
palette-edit-metadata-help = Seçili fotoğrafların başlığını, resim yazısını, oluşturanını, haklarını ve konumlarını düzenleyin.
metaedit-menu = Meta veriyi düzenle…
metaedit-title = Meta veri
metaedit-close = Meta veriyi kapat
metaedit-photos = { $count ->
    [one] 1 fotoğraf
   *[other] { $count } fotoğraf
}
metaedit-none-selected = Meta verisini düzenlemek için fotoğraf seçin.
metaedit-reading = Meta veri okunuyor…
metaedit-mixed = Karışık: hepsine uygulamak için yazın
metaedit-clearing = Tüm fotoğraflarda temizlenir. Esc ile değerler korunur.
metaedit-keywords-others = bazılarında daha fazlası var
metaedit-saved = { $field } kaydedildi.
metaedit-cleared = { $field } temizlendi.
metaedit-write-failed = { $count ->
    [one] 1 fotoğrafın meta verisi kaydedilmedi: { $error }
   *[other] { $count } fotoğrafın meta verisi kaydedilmedi: { $error }
}
metaedit-writes-sidecars = XMP yan dosyalarına kaydedilir.
metaedit-writes-files = JPEG, TIFF ve DNG dosyalarına kaydedilir; diğer dosyalar için XMP yan dosyaları kullanılır.

metaedit-group-templates = Şablonlar
metaedit-group-description = Açıklama
metaedit-group-creator = Oluşturan
metaedit-group-rights = Haklar
metaedit-group-shown = Gösterilen konum
metaedit-group-created = Oluşturulduğu konum
metaedit-group-workflow = İş akışı
metaedit-templates-none = Henüz şablon yok. Şablon, sık kullandığınız alanları doldurur.

metaedit-field-title = Başlık
metaedit-field-headline = Manşet
metaedit-field-description = Resim yazısı
metaedit-field-keywords = Anahtar sözcükler (; ile ayrılmış)
metaedit-field-persons-shown = Gösterilen kişiler (; ile ayrılmış)
metaedit-field-event = Etkinlik
metaedit-field-creator = Oluşturan (; ile ayrılmış)
metaedit-field-creator-job-title = Oluşturanın unvanı
metaedit-field-contact-address = Adres
metaedit-field-contact-city = Şehir
metaedit-field-contact-region = Eyalet veya il
metaedit-field-contact-postal-code = Posta kodu
metaedit-field-contact-country = Ülke
metaedit-field-contact-phone = Telefon
metaedit-field-contact-email = E-posta
metaedit-field-contact-url = Web sitesi
metaedit-field-copyright = Telif hakkı bildirimi
metaedit-field-credit = Atıf satırı
metaedit-field-source = Kaynak
metaedit-field-usage-terms = Kullanım koşulları
metaedit-field-shown-sublocation = Alt konum
metaedit-field-shown-city = Şehir
metaedit-field-shown-state = Eyalet veya il
metaedit-field-shown-country = Ülke
metaedit-field-shown-country-code = Ülke kodu
metaedit-field-created-sublocation = Alt konum
metaedit-field-created-city = Şehir
metaedit-field-created-state = Eyalet veya il
metaedit-field-created-country = Ülke
metaedit-field-created-country-code = Ülke kodu
metaedit-field-instructions = Talimatlar
metaedit-field-job-id = İş kimliği

metaedit-template-new = Yeni şablon
metaedit-template-edit = Düzenle
metaedit-template-remove = Kaldır
metaedit-template-done = Bitti
metaedit-template-name = Şablon adı
metaedit-template-default-name = Şablon { $n }
metaedit-template-title = Şablon
metaedit-template-hint = Boş alanlara dokunulmaz.
metaedit-template-replace-keywords = Fotoğrafların anahtar sözcüklerini değiştir
metaedit-template-applied = { $count ->
    [one] “{ $name }” 1 fotoğrafa uygulandı.
   *[other] “{ $name }” { $count } fotoğrafa uygulandı.
}
metaedit-template-remove-ask = “{ $name }” şablonu kaldırılsın mı?
metaedit-template-remove-keep = Fotoğraflar meta verilerini korur.
metaedit-template-remove-yes = Kaldır
metaedit-template-removed = “{ $name }” şablonu kaldırıldı.

app-info-edit-metadata = Meta veriyi düzenle…

settings-gallery-metadata-into-files = Meta veriyi görüntü dosyalarına yaz
settings-gallery-metadata-into-files-help = Meta veri düzenleyici JPEG, TIFF ve DNG dosyalarına da yazar ve içlerinde başka hiçbir şeyi değiştirmez. Kapalı: yalnızca XMP yan dosyaları.
