# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] Çakışmalar
    [changes] Değişiklikler
   *[quantities] Metrajlar
}
# $tab: the tab's name.
checks-title = Model denetimleri · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] Model denetimleri için bir IFC modeli gerekir.
    [reading] Model bilgileri okunuyor…
    [ids-empty] IDS dosyası yok. Açmak için O’ya basın.
    [ids-checking] Denetleniyor…
    [ids-no-failures] Başarısız olan yok.
    [ids-hints] Enter hatalar, git · Backspace geri · O aç · P özellikler · Ctrl+S BCF · Ctrl+E CSV · Tab sonraki
    [unnamed] Adsız belirtim
    [clash-running] Çakışmalar aranıyor…
    [clash-empty] Çakışmaları aramak için R’ye basın.
    [clash-none] Çakışma yok.
    [clashes-hints] R çalıştır · A B kümeler · M sert veya boşluk · + − mesafe · Enter git · S durum · Ctrl+S BCF · Ctrl+E CSV · Tab sonraki
    [changes-reading] Diğer sürüm okunuyor…
    [changes-empty] Bu modelin başka bir sürümünü seçmek için O’ya basın.
    [changes-none] Değişiklik yok.
    [changes-hints] O diğer sürüm · F filtre · Enter git · P özellikler · Ctrl+S BCF · Ctrl+E CSV · Tab sonraki
    [quantities-hints] G grupla · Enter git · P özellikler · Ctrl+E CSV · Ctrl+S XLSX · Tab sonraki
   *[not-drawn] Bu elemanın gösterilecek geometrisi yok.
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] Sınıf
    [attribute] Öznitelik { $name }
    [classification] Sınıflandırma
    [material] Malzeme
   *[partof] Parçası olduğu: { $name }
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] Sınıf: { $actual }
    [predefined] Önceden tanımlı tür: { $actual }
    [missing] { $what } eksik
    [empty] { $what } boş
    [value] { $what } = { $actual }: izin verilmiyor
    [datatype] { $what }: veri türü { $actual }
    [system] { $what }: sistem { $actual }
    [whole] { $what }: parçası olduğu { $actual }
    [prohibited] { $what } bulunmamalı
   *[unsupported] { $what } denetlenemiyor
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] Kat: { $value }
    [type] Tür: { $value }
    [material] Malzeme: { $value }
    [container] Kapsayıcı: { $value }
   *[classification] Sınıflandırma: { $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }: { $count } belirtimden { $ok } tanesi sağlanıyor

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } geçti
    [fail] ✗ { $applicable } içinden { $failed } başarısız
    [none] ✗ hiçbir şeye uygulanmıyor
    [none-ok] ✓ hiçbir şeye uygulanmıyor
    [prohibited] ✗ bulunmaması gereken { $applicable } öğe
   *[prohibited-ok] ✓ bulunmaması gereken öğe yok
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] tüm elemanlar
    [class] { $name }
   *[storey] kat { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A: { $a } · B: { $b } · { $mode ->
    [hard] sert, { $distance } değerinden derin
   *[clearance] { $distance } değerinden yakın
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
    [one] { $count } çakışma
   *[other] { $count } çakışma
}, { $new } yeni, { $resolved } çözüldü · { $seconds } sn
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] sert { $depth }
   *[clearance] boşluk { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] Sert
   *[clearance] Boşluk
}
checks-status = { $status ->
    [new] yeni
    [active] etkin
   *[resolved] çözüldü
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = Çakışma: { $a } × { $b }
checks-clash-done = { $count ->
    [0] Çakışma yok
    [one] { $count } çakışma
   *[other] { $count } çakışma
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }: { $added } eklendi · { $removed } kaldırıldı · { $changed } değişti{ $filter ->
    [added] {" "}· eklenenler gösteriliyor
    [removed] {" "}· kaldırılanlar gösteriliyor
    [changed] {" "}· değişenler gösteriliyor
   *[all] {""}
}
checks-change = { $kind ->
    [added] Eklendi
    [removed] Kaldırıldı
   *[geometry] geometri
}
checks-change-fields = { $count ->
    [one] { $count } alan
   *[other] { $count } alan
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }: { $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = { $distance } taşındı

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count } eleman · { $area } · { $volume } · { $by ->
    [element] elemana göre
    [class] sınıfa göre
    [storey] kata göre
   *[type] türe göre
}
# Where quantities came from.
checks-source = { $source ->
    [file] dosyadan
    [computed] hesaplandı
   *[mixed] kısmen hesaplandı
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] Elemanlar
    [class] Sınıfa göre
    [storey] Kata göre
   *[type] Türe göre
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] Belirtim
    [guid] GlobalId
    [class] Sınıf
    [name] Ad
    [reason] Neden
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] Çeşit
    [depth] Derinlik veya mesafe (m)
    [status] Durum
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] Değişiklik
    [field] Alan
    [old] Eski
    [new] Yeni
    [type] Tür
    [container] Kapsayıcı
    [material] Malzeme
    [classification] Sınıflandırma
    [storey] Kat
    [count] Sayı
    [length] Uzunluk (m)
    [area] Alan (m²)
    [volume] Hacim (m³)
   *[source] Kaynak
}

# $reason: why, in plain words.
checks-error = Yapılamıyor: { $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] Denetlendi: tüm belirtimler sağlanıyor
    [one] Denetlendi: { $failed } belirtim sağlanmıyor
   *[other] Denetlendi: { $failed } belirtim sağlanmıyor
}
# $file: the file written.
checks-saved = { $file } kaydedildi

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element } ({ $model })
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = tüm modeller
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = Model: { $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M model · Enter hatalar, git · Backspace geri · O aç · P özellikler · Ctrl+S BCF · Ctrl+E CSV · Tab sonraki
    [clashes] R çalıştır · A B kümeler · G H modelleri · M sert veya boşluk · + − mesafe · Enter git · S durum · Ctrl+S BCF · Ctrl+E CSV · Tab sonraki
    [changes] M model · O diğer sürüm · F filtre · Enter git · P özellikler · Ctrl+S BCF · Ctrl+E CSV · Tab sonraki
   *[quantities] M model · G grupla · Enter git · P özellikler · Ctrl+E CSV · Ctrl+S XLSX · Tab sonraki
}
