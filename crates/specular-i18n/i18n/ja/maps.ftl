# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# 日本語.

keys-mode-map = 地図
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = レイヤー
cheatsheet-map-coordinates = 座標
cheatsheet-map-measure = 計測

## Palette entries and their one-line descriptions.
palette-map-layers = レイヤーパネルの表示/非表示
palette-map-layers-help = 地図のレイヤー：順序、不透明度、表示、スタイル。
palette-map-add-layer = ファイルからレイヤーを追加…
palette-map-add-layer-help = シェープファイルに GPX トラックを重ねるなど、別の地図ファイルを上に描きます。
palette-map-zoom-to-layer = レイヤーにズーム
palette-map-zoom-to-layer-help = レイヤーパネルで選んだレイヤー全体を表示します。
palette-map-attribute-table = 属性テーブルの表示/非表示
palette-map-attribute-table-help = 選んだレイヤーの地物を表で表示：並べ替え、絞り込み、検索、選択。
palette-map-coordinate-format = 次の座標形式
palette-map-coordinate-format-help = 十進度、度分秒、UTM、MGRS。
palette-map-units = メートル法 / ヤード・ポンド法
palette-map-units-help = 長さと面積をメートル、またはフィートとマイルで表示します。
palette-map-copy-coordinate = 座標をコピー
palette-map-copy-coordinate-help = ポインター位置、または地図の中心の位置をコピーします。
palette-map-go-to-coordinate = 座標へ移動…
palette-map-go-to-coordinate-help = 任意の形式で位置を入力すると、そこを地図の中心にします。
palette-map-measure-distance = 距離を計測
palette-map-measure-distance-help = 地図上の点をクリック：地球表面上の長さと方位。
palette-map-measure-area = 面積を計測
palette-map-measure-area-help = 範囲の角をクリック：地球表面上の面積と周長。
palette-map-elevation-profile = 標高断面
palette-map-elevation-profile-help = 標高モデルの上に線を引き、その高さをグラフにします。
palette-map-overview = 概観図の表示/非表示
palette-map-overview-help = 全範囲の小さな地図で、画面に表示中の部分を示します。

## Why a map command is unavailable.
disabled-not-map = 地図ツールがオンのときの地図専用です
disabled-no-lonlat = 地図の座標系が不明です
disabled-no-features = 地図に地物がありません
disabled-no-heights = 地図に高さがありません

## Settings › Maps.
settings-section-maps = 地図
settings-maps-coordinate-format = 座標形式
settings-maps-coordinate-format-help = 位置の表示とコピーの形式です。「座標へ移動」はどの形式も読み取れます。
settings-maps-coordinate-format-dd = 十進度
settings-maps-coordinate-format-dms = 度、分、秒
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = 単位
settings-maps-units-help = 地図上の長さと面積：縮尺バーと計測。自動は「メタデータ › 単位」に従います。
settings-maps-units-auto = 自動
settings-maps-units-metric = メートル法
settings-maps-units-imperial = ヤード・ポンド法

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] 緯度, 経度 { $position }
}
map-height = 高さ { $height }
map-height-label = 高さ
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = 縮尺バー：{ $length }
map-north-label = 方位記号：北は上から時計回りに { $degrees }°
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] 度分秒
    [utm] UTM
    [mgrs] MGRS
   *[dd] 十進度
}
map-format-toast = 座標：{ $format }
map-units-metric = 単位：メートルとキロメートル
map-units-imperial = 単位：フィートとマイル
toast-copied-coordinate = 座標をコピーしました

## Go to coordinate.
map-goto-hint = 十進度、DMS、UTM、MGRS。Enter で移動します。
map-goto-empty = 位置を入力してください。
map-goto-unrecognised = 既知の形式の位置ではありません。
map-goto-range = 緯度または経度が範囲外です。
map-goto-no-way = この地図では位置を配置できません。
map-goto-outside = その位置は地図の外です。
map-goto-toast = { $position } を中心にしました

## The layer panel.
map-layers-title = レイヤー：{ $total } 個中 { $shown } 個を表示
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }：{ $kind }、{ $features ->
   *[other] 地物 { $features } 個
}、不透明度 { PERCENT($opacity) }
map-kind-area = 面
map-kind-line = 線
map-kind-point = 点
map-kind-empty = 地物なし
map-layer-visible = レイヤーの表示/非表示
map-layer-opacity-down = 不透明度を下げる
map-layer-opacity-up = 不透明度を上げる
map-layer-colour = 次の色
map-layer-width = 次の線幅
map-layer-point = 次の点サイズ
map-layer-up = 上へ移動
map-layer-down = 下へ移動
map-layer-remove = 削除
map-layer-keep = 残す
map-layer-remove-ask = { $name } を地図から削除しますか？ファイルはディスクに残ります。
map-layer-adding = レイヤーを追加中…
map-removed = { $name } を地図から削除しました
map-added = { $layers ->
   *[other] { $name } を追加しました：{ $layers } レイヤー
}
map-add-failed = { $name } を追加できません：{ $reason }
map-add-filter = 地図ファイル
map-add-not-map = 地図ではありません。
map-add-not-vector = 地物がありません。
map-add-no-crs = 投影法ファイル (.prj) がありません。

## Identify.
map-identify-more = ほか { $count } 件は属性テーブルにあります
map-cell = セル { $x }, { $y }
map-band = バンド { $n }
map-no-data = データなし

## The attribute table.
map-table-title = { $layer }：地物 { $total } 個中 { $shown } 個、{ $selected } 個選択
map-table-search = 検索、またはフィールド = 値
map-table-selected-only = 選択のみ
map-table-empty = 一致する地物はありません。
map-table-many = 多数
map-table-stats-number = { $field }：値 { $count } 個、空 { $empty } 個 · 最小 { $min } · 最大 { $max } · 平均 { $mean } · 合計 { $sum } · 標準偏差 { $sd }
map-table-stats-text = { $field }：値 { $count } 個、空 { $empty } 個 · 異なる値 { $distinct } 個
map-table-stats-text-top = { $field }：値 { $count } 個、空 { $empty } 個 · 異なる値 { $distinct } 個 · 最頻 { $top } ({ $times }×)

## Measuring and the elevation profile.
map-measure-hint-distance = 経路に沿って点をクリックします。
map-measure-hint-area = 範囲の角をクリックします。
map-measure-hint-profile = 線に沿って点をクリックします。
map-measure-keys = Enter で完了、Backspace で最後の点を削除、Esc で閉じます。
map-measure-distance = 距離 { $length }
map-measure-bearing = 方位 { $bearing }
map-measure-segment = 最後の区間 { $length }、方位 { $bearing }
map-measure-area = 面積 { $area }
map-measure-perimeter = 周長 { $length }
map-bearing-value = { $degrees }°
map-profile-summary = 長さ { $length } · 最低 { $min } · 最高 { $max } · 登り { $climb } · 下り { $descent }
map-profile-no-heights = 長さ { $length }。線上に高さがありません。
map-profile-at = { $distance }：{ $height }
map-profile-at-none = { $distance }：高さなし
