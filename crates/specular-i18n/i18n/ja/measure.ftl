# Specular — 計測ツール (任意の画像での長さ、角度、領域とその統計)。機械翻訳の下書き、ネイティブによる確認が必要。

palette-measure = 計測
palette-measure-copy-csv = 計測を CSV としてコピー
palette-measure-export = 計測を書き出す…
toast-copied-measurements = 計測を CSV としてコピーしました

measure-tool-title = 計測
measure-tool-list = 計測
measure-tool-count = { $count ->
    [0] 計測なし
   *[other] { $count } 件の計測
}
measure-tool-empty = ツールを選んで画像上に描いてください。

measure-tool-point = 点
measure-tool-line = 直線
measure-tool-polyline = 折れ線
measure-tool-angle = 角度
measure-tool-rect = 長方形
measure-tool-ellipse = 楕円
measure-tool-polygon = 多角形
measure-tool-freehand = フリーハンド
measure-tool-calibrate = 較正
# machine-drafted, needs native review
measure-tool-spot = スポット

measure-tool-undo = 取り消す
measure-tool-delete = 削除
measure-tool-copy = CSV をコピー
measure-tool-export = エクスポート…
measure-tool-import = 読み込む…
measure-tool-done = 完了
measure-tool-unit = 単位: { $unit }
measure-tool-apply = 適用

measure-tool-hint-line = ドラッグするか 2 点をクリック。矢印キーで十字線を移動、スペースで点を置きます。
measure-tool-hint-angle = 3 点をクリック (2 点目が頂点)。
measure-tool-hint-rect = ドラッグするか 2 つの角をクリック。
measure-tool-hint-ellipse = ドラッグするか外接枠の 2 つの角をクリック。
measure-tool-hint-polygon = 角をクリック。Enter か最初の角のクリックで閉じます。
measure-tool-hint-calibrate = 長さのわかっているものに沿って線を引きます。
# machine-drafted, needs native review
measure-tool-hint-spot = 点をクリックすると、その温度を読み取ります。
measure-tool-known-length = 既知の長さ: { $value } { $unit } (数値を入力、U で単位を変更、Enter で適用)
measure-tool-calibrated = 較正済み: 1 ピクセル = { $size } { $unit }
measure-tool-bad-length = 0 より大きい長さを入力してください。

measure-tool-item = { $n }. { $shape }
measure-tool-area = 面積 { $value }
measure-tool-mean = 平均 { $mean } ± { $sd }
measure-tool-means = 平均 { $values }
measure-tool-range = 範囲 { $min } – { $max } ({ $count } px)
measure-tool-pixels = { $count } px
measure-tool-stats-pending = ピクセルを読み取り中…
measure-tool-stats-none = ここにはピクセル値がありません
measure-tool-approximate = 縮小コピーから (概算)

measure-tool-deleted = 計測を削除しました
measure-tool-undone = 取り消しました
measure-tool-nothing = この画像に計測はありません
measure-tool-exported = 計測を { $name } に保存しました
measure-tool-export-failed = 計測を保存できませんでした: { $reason }
measure-tool-imported = { $count ->
   *[other] { $count } 件の計測を読み込みました。
}
measure-tool-skipped = { $count ->
   *[other] { $count } 個の図形を読み取れませんでした。
}
measure-tool-import-failed = 計測を読み取れませんでした: { $reason }
measure-tool-too-large = ファイルが大きすぎます。
# machine-drafted, needs native review
measure-tool-sr-not-dicom = DICOM SR には DICOM 画像が必要です。代わりに CSV または GeoJSON で保存してください。

measure-source-uncalibrated = ピクセル (未較正)
measure-source-dicom-pixel-spacing = DICOM ピクセル間隔
measure-source-dicom-calibrated = DICOM 較正済み間隔
measure-source-dicom-imager-pixel-spacing = DICOM 検出器上の間隔
measure-source-dicom-magnification = DICOM 拡大率補正済み間隔
measure-source-dicom-ultrasound-region = DICOM 超音波領域
measure-source-tiff-resolution = TIFF 解像度
measure-source-exif-resolution = EXIF 解像度
measure-source-ome-physical-size = OME 物理サイズ
measure-source-fits-wcs = FITS 天球座標
measure-source-vendor-tag = ベンダーのメタデータ
measure-source-drawing-units = 図面の単位
# machine-drafted, needs native review
measure-source-drawing-scale = 図面の縮尺から
# machine-drafted, needs native review
measure-source-drawing-scale-ratio = 図面から（縮尺 { $scale }）
# machine-drafted, needs native review
measure-source-drawing-scale-viewport = 図面から（縮尺 { $scale }、ビューポート { $viewport }）
# machine-drafted, needs native review
measure-source-map-scale = 地図の縮尺（中心での近似値。地図ツールは地球上で計測します）
measure-source-user = ユーザーの較正
