# machine-drafted, needs native review
# Photographer polish pass: Develop regions, Crop card rows, import choices, keyword rename and move, auto-apply rules.

# Develop: F6 between the picture, the panel and the filmstrip.
palette-develop-next-region = Sonraki bölge
palette-develop-next-region-help = Klavyeyi ileri taşı: görüntü, panel, film şeridi.
palette-develop-prev-region = Önceki bölge
palette-develop-prev-region-help = Klavyeyi geri taşı: film şeridi, panel, görüntü.
develop-region-picture = Görüntü
develop-region-panel = Ayar paneli

# Develop: the Crop card's rows (the crop tool's keys as rows).
develop-crop-aspect-row = En boy oranı
develop-crop-aspect-option-free = Serbest
develop-crop-aspect-option-original = Özgün
develop-crop-aspect-option-custom = Özel
develop-crop-type-ratio = Oran yaz…
develop-crop-swap = Dikey ve yatayı değiştir
develop-crop-turn-left = Sola döndür
develop-crop-turn-right = Sağa döndür
develop-crop-flip-horizontal = Soldan sağa çevir
develop-crop-flip-vertical = Yukarıdan aşağıya çevir
develop-crop-ratio-title = Kırpma oranı
develop-crop-ratio-field = Genişlik:yükseklik
develop-crop-ratio-ok = Ayarla
develop-crop-ratio-bad = 5:4 gibi bir oran ya da 1.85 gibi tek bir sayı yazın.

# Import: what each copied photo gets.
import-template = Meta veri şablonu
import-style = Geliştirme stili
import-choice-none = Yok
# A choice row stepped, read out: "Metadata template: Copyright".
import-choice-said = { $label }: { $value }
import-keywords = Anahtar sözcükler
import-keywords-hint = Örneğin Aile; Yerler > Paris
import-style-missing = { $name } stili artık yok. Fotoğraflar o stil olmadan içe aktarıldı.

# Keywords: rename, move and merge.
keywords-rename = Yeniden adlandır…
keywords-move = Taşı…
keywords-rename-of = { $keyword } için yeni ad
keywords-move-of = { $keyword } şunun altına (boş: en üst düzey)
keywords-move-into-itself = { $keyword } kendi altına taşınamaz.
keywords-renaming = { $from }, { $to } olarak yeniden adlandırılıyor…
keywords-merging = { $from }, { $to } ile birleştiriliyor…
keywords-renamed = { $count ->
    [one] { $count } fotoğrafta { $from } artık { $to }
   *[other] { $count } fotoğrafta { $from } artık { $to }
}
keywords-rename-row = { $from } anahtar sözcüğü → { $to }
keywords-rename-finding = Fotoğraflar aranıyor…
keywords-rename-row-detail = { $done } / { $total } fotoğraf

# Develop: the before/after divider dragged ("Divider at 30 %").
develop-split-moved = Ayırıcı { PERCENT($percent) } konumunda

# Develop: auto-apply rules, changed row by row.
develop-rules-placeholder-edit = Değiştirmek veya silmek için bir kural seçin
develop-rule-edit-placeholder = { $rule }
develop-rule-maker = Üretici: { $value }
develop-rule-model = Model: { $value }
develop-rule-lens = Objektif: { $value }
develop-rule-iso = ISO: { $value }
develop-rule-type = Dosya türü: { $value }
develop-rule-style = Stil: { $value }
develop-rule-any = herhangi biri
develop-rule-type-any = herhangi bir dosya
develop-rule-delete = Kuralı sil…
develop-rule-field-title = “{ $rule }” kuralını değiştir
develop-rule-field-maker = Üretici (adın bir kısmı yeterli; boş: herhangi biri)
develop-rule-field-model = Model (adın bir kısmı yeterli; boş: herhangi biri)
develop-rule-field-lens = Objektif (adın bir kısmı yeterli; boş: herhangi biri)
develop-rule-field-iso = ISO, örneğin 100-800, 1600- veya -400 (boş: herhangi biri)
develop-rule-field-ok = Ayarla
develop-rule-iso-bad = 100-800 gibi bir ISO aralığı yazın.
develop-rule-style-placeholder = Kuralın stilini seçin
