# Multiplanar reformatting (MPR).

palette-mpr = 多断面再構成 (MPR)
palette-mpr-slab = 次の MPR スラブ (薄層、MIP、MinIP、平均)
disabled-no-volume = これはボリュームではありません。シリーズ、マルチフレームファイル、3D ボリューム、または Z スタックを開いてください
disabled-mpr-closed = 先に多断面ビューを開いてください
mpr-axial = 軸位
mpr-coronal = 冠状
mpr-sagittal = 矢状
mpr-oblique = 斜位
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D ビュー
mpr-thin = 薄層
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = 平均
mpr-slab = スラブ: { $mode }、{ $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = スラブ: { $mode }、{ $thickness } Å
mpr-slab-unit = スラブ: { $mode }、{ $thickness } { $unit }
mpr-slab-voxels = スラブ: { $mode }、{ $thickness } ボクセル
mpr-thicker = スラブを厚く
mpr-thinner = スラブを薄く
mpr-reset = ビューをリセット
mpr-close = MPR を閉じる
mpr-maximise = ビューを最大化
mpr-export-planes = 断面を書き出す
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = { $files } のストリームライン: スラブ { $thickness } mm
mpr-tracts-off = { $files } のストリームライン: 非表示
mpr-tracts-none = このボリュームの横に TrackVis や MRtrix のストリームラインがありません
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } ボクセル
mpr-axis-label = { $axis }: { $label }
mpr-axis-position = { $axis }: { $position } / { $count }
mpr-axis-channel = チャンネル
mpr-axis-unnamed = 軸 { $axis }
mpr-one-channel = MPR では一度に 1 チャンネルずつ表示します
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } チャンネルをカラーで表示 (それぞれの表示範囲)
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = 1 チャンネルのみ表示: { $reason }
mpr-isotropic = ファイルにボクセルサイズがありません。ボクセルを立方体として表示します
mpr-isotropic-z = ファイルにスライス間隔がありません。スライスを 1 ピクセル間隔で表示します
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = インライン { $inline }
mpr-seismic-crossline = クロスライン { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = { $count } 枚中 { $loaded } 枚のスライスを読み込み中
mpr-loaded = { $count } スライス、{ $width } × { $height }
mpr-downsampled = 1/{ $factor } の解像度で表示中 (メモリまたはグラフィックスの制限)
mpr-converted = 値を 32 ビット浮動小数点で表示
mpr-opening = ボリュームを開いています…
mpr-gpu-memory = グラフィックカード上の { $size }
mpr-refused = ボリュームを構築できません: { $reason }
mpr-failed = ボリュームを読み込めませんでした: { $reason }
mpr-no-gpu = グラフィックカードにこのボリュームを保持できません: { $reason }
mpr-planes-saved = { $count } 枚の断面を { $folder } に保存しました
mpr-planes-failed = 断面を保存できませんでした: { $reason }
mpr-view-a11y = { $view } ビュー、{ $position } mm
mpr-crosshair-a11y = 十字線 { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = 十字線 { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = 十字線 { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = 十字線 ボクセル { $x }, { $y }, { $z }
mpr-controls = MPR コントロール

# 3D view palette entries.
palette-mpr-preset = 次の 3D プリセット (CT 骨、軟部組織、肺、血管造影、MR)
palette-mpr-render = 次の 3D レンダリング (シェーディング、Cinematic、MIP、MinIP)
palette-mpr-clip = 次の 3D クリッピング (なし、2D ビューのボックス、角の切り欠き)
palette-mpr-export-stl = 3D サーフェスを STL として書き出す…
# machine-drafted, needs native review
palette-mpr-tract-slab = MPR の次のストリームラインスラブ
palette-mpr-maximise = MPR ビューを最大化、またはすべてのビューを表示
palette-mpr-export-planes = MPR の断面を画像として書き出す…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] CT 骨
    [soft] CT 軟部組織
    [lung] CT 肺
    [angio] CT 血管造影
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] シェーディング
}
vr-clip = { $clip ->
    [box] クリップ: ビューのボックス
    [cut] クリップ: 角の切り欠き
   *[off] クリップなし
}
vr-smooth = サーフェスを平滑化
vr-export = STL を書き出す
vr-threshold = サーフェス閾値 { $value }
vr-stl-exporting = サーフェスを作成しています…
vr-stl-saved = { $count ->
    [0] この閾値には何もありません: サーフェスは保存されませんでした
   *[other] { $count } 個の三角形を { $name } に保存しました
}
vr-stl-failed = サーフェスを保存できませんでした: { $reason }
vr-view-a11y = { $preset }、{ $mode }、{ $clip }、サーフェス閾値 { $threshold }。Shift+矢印キーで回転、プラスとマイナスでズーム。

## RAW ボリューム: ヘッダーのない .raw / .vol ファイルのデータ配置

# machine-drafted, needs native review
raw-volume-title = RAW ボリュームを開く
raw-volume-file = { $file } にはヘッダーがありません。サンプルの配置を指定してください。
raw-volume-guess-label = 推定された配置
raw-volume-guess = { $index } / { $count }
raw-volume-custom = カスタム
raw-volume-width = 幅 (ボクセル)
raw-volume-height = 高さ (ボクセル)
raw-volume-slices = スライス数
raw-volume-type = サンプル型
raw-volume-type-unsigned = 符号なし { $bits } ビット
raw-volume-type-signed = 符号付き { $bits } ビット
raw-volume-type-float = 浮動小数点 { $bits } ビット
raw-volume-order = バイト順
raw-volume-little = リトルエンディアン
raw-volume-big = ビッグエンディアン
raw-volume-offset = ヘッダーオフセット
raw-volume-bytes = { $count ->
   *[other] { $count } バイト
}
raw-volume-pixel = ピクセルサイズ
raw-volume-spacing = スライス間隔
raw-volume-mm = { $value } mm
raw-volume-exact = ファイルにぴったり収まります ({ $size })
raw-volume-fits = { $file } のうち { $size } を使用、{ $left } が余ります
raw-volume-too-big = { $size } 必要ですが、ファイルは { $file } です
raw-volume-cancel = キャンセル
raw-volume-open = 開く
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = { $seed } の結合性。十字線を別のシードへ移動してください
# machine-drafted, needs native review
mpr-measure-hint-edit = 端をドラッグして移動、もう一方の端に重ねると削除
# machine-drafted, needs native review
mpr-measure-removed = 計測を削除しました
mpr-measure-lengths-only = MPR で測れるのは長さだけです
mpr-more = その他…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = クリックした曲線に沿った曲面 MPR
palette-mpr-curve-layout = 曲面 MPR: 直線化または伸展
palette-mpr-curve-save = 曲線を保存…
palette-mpr-curve-load = 曲線を読み込む…
palette-mpr-export-segments = 各セグメントを STL として書き出す…
palette-mpr-dental = 歯科レイアウト（パノラマとクロスセクション）
palette-mpr-nerve = 下顎管をトレース
cpr-kind = { $kind ->
    [stretched] 伸展
   *[straightened] 直線化
}
cpr-button = 曲面
cpr-controls = 曲面 MPR の操作
cpr-at = 位置 { $position }
cpr-section = クロスセクション
cpr-hint-start = { $view } ビューで構造に沿って点をクリックしてください
cpr-hint-more = もう 1 点クリックしてください
cpr-add-points = 点を追加
cpr-undo-point = 点を取り消す
# machine-drafted, needs native review
cpr-mark = ここにマーク
cpr-mark-set = マーク { $n } を設定しました。
cpr-mark-removed = マークを削除しました。
cpr-mark-here = マーク { $n }
cpr-diameter = 直径 { $value }
cpr-diameters = 直径 { $min }～{ $max }
cpr-mark-span = マーク { $from }～{ $to }: 中心線に沿って { $length }
cpr-mark-total = すべてのマーク: { $length }
cpr-clear = 曲線を消去
cpr-save = 曲線を保存
cpr-load = 曲線を読み込む
cpr-done = 完了
cpr-a11y = { $name }、長さ { $length }、カーソル位置 { $position }。ホイールでカーソルを移動します。
cpr-none = 曲線がまだありません: 先に点をクリックしてください
cpr-cleared = 曲線を消去しました
cpr-saved = 曲線を保存しました: { $name }
cpr-save-failed = 曲線を保存できませんでした: { $reason }
cpr-loaded = 曲線を読み込みました
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
   *[other] { $count } 本の中心線を読み込みました
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
   *[other] 保存済みの中心線が { $count } 本見つかりました
}
# A centreline without a name, by its place in the list.
cpr-centreline = 中心線 { $number }
cpr-new = 新しい中心線
cpr-rename = 名前の変更
cpr-name = 中心線の名前
cpr-delete = 削除
# $name: the centreline's name.
cpr-delete-title = 中心線「{ $name }」を削除しますか？
cpr-delete-detail = その点は消えます。ディスクに保存したファイルは残ります。
cpr-deleted = 「{ $name }」を削除しました
cpr-load-failed = 曲線を読み込めませんでした: { $reason }
cpr-load-not-json = ファイルが JSON ではありません
cpr-load-not-curves = ファイルが Specular の曲線ではありません
cpr-load-empty = ファイルに 2 点以上の曲線がありません
cpr-panoramic = パノラマ
cpr-panoramic-slab = パノラマ、スラブ { $thickness } mm
cpr-panoramic-thin = パノラマ、薄層

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = 歯科
dental-not-for-diagnosis = 診断や治療計画には使用できません
dental-nerve = 下顎管
dental-nerve-hint = パノラマまたはクロスセクション上で下顎管に沿って点をクリックしてください
dental-nerve-done = 下顎管のトレースをオフにしました
dental-spacing = { $spacing } mm 間隔
dental-section = 断面 { $number }
dental-off = 先に歯科レイアウトを開いてください
dental-suggesting = 歯列弓を検出しています…
dental-suggested = 歯列弓を提案しました: 点をドラッグして調整してください
dental-no-arch = 歯列弓が見つかりません: 軸位ビューで歯列弓に沿って点をクリックしてください
dental-vendor = { $vendor } 歯科用 CBCT

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = セグメントを書き出す
seg-stl-detail = セグメント: { $count } 個の三角形
seg-stl-detail-full = セグメント: 最大精細
seg-stl-none = 書き出すセグメントがありません
seg-stl-not-labels = このボリュームはラベルマップではありません
seg-stl-exporting = セグメントのサーフェスを作成しています…
seg-stl-saved = { $count } 個のセグメントを { $folder } に保存しました
seg-stl-open = 閉じていないもの: { $count }。
seg-stl-empty = セグメントが空です: 何も保存されませんでした
seg-stl-failed = セグメントを保存できませんでした: { $reason }
# machine-drafted, needs native review
toolbar-mpr = 3 断面 (MPR)
toolbar-mpr-close = 3 断面を閉じる (MPR)
settings-viewing-toolbar-items-mpr = 3 断面 (MPR)
