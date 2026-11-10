# machine-drafted, needs native review
# Specular — 病理: 染色分離、注釈の書き出しと読み込み、ボリュームに重ねるラベルボリューム。日本語。

## パレット
palette-stain-separation = 染色を分離
palette-stain-1 = 染色 1 の表示/非表示
palette-stain-2 = 染色 2 の表示/非表示
palette-stain-3 = 染色 3 の表示/非表示
palette-stain-mode = 染色を光または光学濃度で表示
palette-stain-estimate = 領域から染色を推定
palette-stain-normalise = 染色の色を正規化
palette-stain-reference = 領域を染色の基準に使う
palette-stain-reset = 公開済みの染色ベクトル
palette-export-annotations = 注釈を書き出す…
palette-import-annotations = 注釈を読み込む…
palette-label-volume = ラベルボリュームを重ねる…

## 無効の理由
disabled-not-colour-image = カラー画像が必要です
disabled-stains-off = 先に染色分離をオンにしてください
disabled-no-region = 先に領域を描いてください
disabled-no-slide-annotations = 書き出す注釈、領域、点がありません
disabled-no-label-target = 多断面ビューでボリュームを開くか、医用画像を開いてください

## 染色名
stain-hematoxylin = ヘマトキシリン
stain-eosin = エオジン
stain-dab = DAB
stain-residual = 残差
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## 染色の通知と凡例
stain-off = 染色分離オフ
# { $set }: H&E、H-DAB、または H&E-DAB。
stain-on = 染色: { $set }
stain-shown = { $stain } を表示
stain-hidden = { $stain } を非表示
stain-mode-light = 染色を光で表示
stain-mode-od = 染色を光学濃度で表示
# 凡例の見出し: 「H&E · 光」「H-DAB · 光学濃度」。
stain-legend-light = { $set } · 光
stain-legend-od = { $set } · 光学濃度
stain-legend-estimated = 領域から推定
stain-legend-normalised = 正規化済み
stain-legend-published = 公開済みベクトル
# { $pixels }: 使った染色ピクセルの数。
stain-estimated = { $pixels } ピクセルから染色を推定しました
stain-estimate-failed = 染色を推定できません: { $reason }
stain-estimate-background = 領域のほとんどが背景です
stain-estimate-one-colour = 領域の色が 1 色だけです
stain-estimate-three = 推定には H&E または H-DAB が必要です
stain-estimate-no-pixels = 領域のピクセルがまだ読み込まれていません
stain-reference-set = 領域を染色の基準に設定しました
stain-normalise-on = 染色の色を正規化しました
stain-normalise-off = 染色の色はスキャンのまま
stain-reset-done = 公開済みの染色ベクトル

## 注釈の書き出しダイアログ
annot-export-title = 注釈を書き出す
annot-export-format = 形式
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = ラベルマスク
annot-export-output = マスクの形式
annot-export-output-png = PNG タイル
annot-export-output-tiff = TIFF タイル
annot-export-output-ome = OME-TIFF ピラミッド
annot-export-region = 範囲
annot-export-region-slide = スライド全体
annot-export-region-view = 表示中の範囲
annot-export-region-drawn = 最後に描いた領域
annot-export-level = レベル
# { $level }: 0 はフル解像度。{ $width } × { $height } はそのピクセルサイズ。
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = タイルサイズ
annot-export-empty = 空のタイル
annot-export-empty-skip = 省く
annot-export-empty-keep = 書き出す
annot-export-what = { $count ->
   *[other] { $count } 個の図形
}、{ $classes ->
   *[other] { $classes } 個のクラス
}
annot-export-lines = 線は COCO とマスクには含まれません。
annot-export-export = エクスポート…
annot-export-writing = 書き出し中…
# { $files }: 書き出したファイル数。
annot-export-done = { $files ->
   *[other] { $files } 個のファイルを書き出しました
}
annot-export-failed = 書き出しに失敗しました: { $reason }
annot-import-done = { $count ->
   *[other] { $count } 件の注釈を読み込みました
}
annot-import-failed = 読み込めません: { $reason }
annot-import-skipped = { $count ->
   *[other] { $count } 個の図形に使える輪郭がありませんでした
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## ラベルボリューム
label-volume-file = ラベルボリューム
label-volume-table = カラーテーブル
# { $file }: ラベルボリュームのファイル名。{ $count }: ラベル数。
label-volume-loaded = { $count ->
   *[other] { $file }: { $count } 個のラベル
}
label-volume-failed = { $file } を重ねられません: { $reason }
label-volume-outside = { $file } はこのボリュームと重なりません
label-volume-not-whole = 値が整数ではありません
label-volume-colour = ラベルではなく色を含んでいます
label-volume-empty = ラベルがありません
label-volume-too-many = ラベルが多すぎます
label-volume-too-large = 大きすぎて再サンプリングできません
label-volume-degenerate = 向きが無効です
# カラーテーブルに名前のないラベル。
label-volume-unnamed = ラベル { $value }
