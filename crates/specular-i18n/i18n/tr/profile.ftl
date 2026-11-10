# Specular — Profiles: what Specular is used for (the first-run question, Settings → Profile,
# the toolbar chip that turns a profile's tools on). Türkçe.

settings-section-profile = Profil
settings-profile-capabilities = { -app-name } uygulamasını ne için kullanıyorum
settings-profile-capabilities-help = Her seçim kendi araçlarını ekler: modlar, komutlar, araç çubuğu düğmeleri ve Ayarlar sayfaları. Genel her zaman açıktır ve her biçim seçimden bağımsız olarak açılır.
settings-profile-capabilities-general = Genel
settings-profile-capabilities-engineering = Mühendislik
settings-profile-capabilities-medical = Tıp
settings-profile-capabilities-science = Bilim
# machine-drafted, needs native review
settings-profile-capabilities-maps = Haritalar (CBS)
settings-profile-shown = Zaten sunuldu
settings-profile-shown-help = İlk çalıştırma sorusu ve daha önce gösterilen “Her zaman” teklifleri; bunlar yeniden gösterilmez.
# The first-run sheet's title; the choices are the settings-profile-capabilities-* labels.
profile-question = { -app-name } uygulamasını ne için kullanacaksınız?
profile-done = Devam
# Under the first-run sheet's title: what a choice does.
profile-intro = Her seçim kendi araçlarını ekler: modlar, komutlar, araç çubuğu düğmeleri ve Ayarlar sayfaları. Her dosya seçiminizden bağımsız olarak açılır.
# Under each choice of the first-run sheet: what it is for.
# machine-drafted, needs native review ([maps] variant)
profile-about = { $capability ->
    [engineering] Teknik çizimler ve 3D modeller.
    [medical] DICOM tetkikleri ve tıbbi hacimler, ölçüm ve füzyon ile.
    [science] Astronomi, mikroskopi ve bilimsel veriler.
    [maps] Haritalar, GPS izleri ve yükseklik modelleri.
   *[general] Fotoğraflar ve gündelik görseller. Her zaman açık.
}
# Above Continue on the first-run sheet.
profile-later = İkisini de istediğiniz zaman değiştirebilirsiniz: profili Ayarlar › Profil, başlangıcı Ayarlar › Genel bölümünden.
# The toolbar chip shown while a medical (or science, engineering) image is on screen.
# machine-drafted, needs native review ([maps] variant)
profile-tools = { $capability ->
    [medical] Tıp araçları
    [science] Bilim araçları
    [maps] Harita araçları
    [engineering] Mühendislik araçları
   *[general] Genel araçlar
}
# Why a command of a profile that is off is greyed out in the palette.
# machine-drafted, needs native review ([maps] variant)
profile-tools-off = { $capability ->
    [medical] Tıp araçları kapalı
    [science] Bilim araçları kapalı
    [maps] Harita araçları kapalı
    [engineering] Mühendislik araçları kapalı
   *[general] Genel araçlar kapalı
}
# Toast after the chip turned the tools on; its button keeps them on.
# machine-drafted, needs native review ([maps] variant)
profile-tools-on = { $capability ->
    [medical] Tıp araçları bu oturum için açık
    [science] Bilim araçları bu oturum için açık
    [maps] Harita araçları bu oturum için açık
    [engineering] Mühendislik araçları bu oturum için açık
   *[general] Genel araçlar bu oturum için açık
}
profile-always = Her zaman
# Palette entries; $tools is a profile-tools label.
profile-turn-on = { $always ->
    [yes] { $tools }: her zaman aç
   *[no] { $tools }: bu oturum için aç
}
