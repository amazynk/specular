# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Albümler
albums-new = Yeni albüm…
albums-new-smart = Yeni akıllı albüm…
albums-save-search = Aramayı akıllı albüm olarak kaydet…
albums-smart = Akıllı albüm
# The album the photos go to when added without choosing.
albums-target = Eklenenler buraya gelir
albums-group-count = { $count ->
   *[other] { $count } albüm
}
albums-need-library = Albümler Kitaplık’tadır. Önce bu klasörü Kitaplık’a ekleyin.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = { $album } albümünü yeniden adlandır
albums-a11y-edit = { $album } albümünü düzenle
albums-a11y-delete = { $album } albümünü sil
albums-a11y-rename-group = { $group } grubunu yeniden adlandır
albums-a11y-delete-group = { $group } grubunu sil

## Sheets
albums-name = Ad
albums-unnamed = Henüz ad yok
albums-new-title = Yeni albüm
albums-new-smart-title = Yeni akıllı albüm
albums-edit-smart-title = Akıllı albümü düzenle
albums-rename-title = Albümü yeniden adlandır
albums-rename-group-title = Grubu yeniden adlandır
albums-create = Oluştur
albums-rename = Yeniden adlandır
albums-save = Kaydet
albums-groups-note = Bir gruba koymak için / kullanın: Müşteri X / seçilenler.
albums-name-empty = Bir ad yazın.
albums-name-taken = { $album } zaten var.
albums-name-clash = { $album } bir albüm olduğundan grup da olamaz.

## Smart album rules
albums-rules = Kurallar
albums-rules-note = Kurallar yukarıdan aşağıya uygulanır. Boş kurallar atlanır.
albums-rules-none = En az bir kural ekleyin.
albums-rule-label = Kural { $number }
albums-rule-empty = Boş kural
albums-rule-add = Kural ekle
albums-rule-combine = Yukarıdaki kurallarla birleştir
albums-rule-first = Şuna uyan fotoğraflar
albums-rule-and = Ve
albums-rule-or = Veya
albums-rule-except = Hariç

## Adding and removing photos
albums-add-menu = Albüme ekle…
albums-remove-menu = Albümden kaldır
albums-add-title = { $count ->
   *[other] { $count } fotoğrafı bir albüme ekle
}
albums-add = Ekle
albums-add-new = Yeni albüm
albums-add-list = Albümler
albums-add-none = Henüz albüm yok. Bir tane oluşturmak için ad yazın.
albums-add-note = Ya da yeni bir albüm için ad yazın.
albums-added = { $count ->
   *[other] { $count } fotoğraf { $album } albümüne eklendi.
}
albums-added-missing = { $count ->
   *[other] { $count } fotoğraf Kitaplık’ta olmadığından eklenmedi.
}
albums-created = { $album } oluşturuldu.
albums-removed = { $count ->
   *[other] { $count } fotoğraf { $album } albümünden kaldırıldı.
}
albums-remove = Kaldır
albums-remove-confirm = { $count ->
   *[other] { $count } fotoğraf { $album } albümünden kaldırılsın mı?
}
albums-remove-confirm-detail = Dosyalar diskte ve Kitaplık’ta kalır.
albums-delete = Sil
albums-delete-confirm = { $album } albümü silinsin mi?
albums-delete-group-confirm = { $count ->
   *[other] { $group } grubu ve içindeki { $count } albüm silinsin mi?
}
albums-delete-confirm-detail = Fotoğraflar diskte ve Kitaplık’ta kalır.
albums-deleted = { $count ->
    [1] Albüm silindi.
   *[other] { $count } albüm silindi.
}
albums-failed = Albümler değiştirilemedi: { $reason }

## Date timeline
albums-timeline = Zaman çizelgesi
albums-timeline-description = Yalnızca o dönemin fotoğraflarını göstermek için bir dönem seçin. Shift+tıklama art arda birkaçını seçer.
albums-timeline-picked = { $period } gösteriliyor
albums-timeline-cleared = Tüm tarihler gösteriliyor
