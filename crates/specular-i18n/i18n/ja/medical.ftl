# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = 医用画像
palette-window-level = ウィンドウ/レベル ツール (左ドラッグ)
palette-window-default = ウィンドウ: ファイルの既定値
palette-window-lung = ウィンドウ: 肺野 (W 1500, L −600)
palette-window-mediastinum = ウィンドウ: 縦隔 (W 350, L 50)
palette-window-bone = ウィンドウ: 骨 (W 1800, L 400)
palette-window-brain = ウィンドウ: 脳 (W 80, L 40)
palette-window-stroke = ウィンドウ: 脳梗塞 (W 40, L 40)
palette-window-soft-tissue = ウィンドウ: 軟部組織 (W 400, L 50)
palette-window-liver = ウィンドウ: 肝臓 (W 150, L 30)
palette-window-subdural = ウィンドウ: 硬膜下 (W 200, L 75)
palette-window-full-range = ウィンドウ: 全範囲
palette-invert-grey = グレーを反転
palette-next-slice = 次のフレーム
palette-prev-slice = 前のフレーム
disabled-not-medical = グレースケールの医用画像のみ
disabled-one-frame = 画像のフレームは 1 つです
settings-section-medical = 医用画像
settings-medical-keep-window = 次のシリーズでもウィンドウ/レベルを保持
settings-medical-keep-window-help = 別のシリーズに移るとき、その既定値ではなく設定したウィンドウを保持します。1 つのファイルのフレーム間と 1 つのシリーズのスライス間では常に共有されます。
medical-window = C { $center }  W { $width }
medical-inverted = 反転
medical-frame = フレーム { $frame } / { $count }
medical-tool = ウィンドウ/レベル ツール: ドラッグ
medical-tool-on = ウィンドウ/レベル ツール オン: ドラッグして調整
medical-tool-off = ウィンドウ/レベル ツール オフ
medical-preset-not-ct = CT プリセットにはハウンスフィールド単位の値が必要です
medical-file-window-none = このファイルにウィンドウ { $index } はありません
medical-file-window-unnamed = ファイルのウィンドウ { $index }/{ $count }
medical-file-window = ファイルのウィンドウ { $index }/{ $count }: { $name }
measure-templates-need-medical = 幾何テンプレートは医用プロファイルで使えます
medical-off = この画像では医用ツールを使えません: { $reason }
medical-readout = 値とウィンドウ

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = フレームを再生 (シネ)
palette-measure-length = 長さを計測
palette-measure-angle = 角度を計測
palette-measure-ellipse = 楕円領域を計測
palette-measure-rectangle = 矩形領域を計測
palette-clear-measurements = 計測を消去
palette-measure-templates = 幾何テンプレート…
palette-overlays = オーバーレイの表示/非表示
medical-cine = { $rate } フレーム/秒で再生中
medical-overlays-shown = オーバーレイを表示
medical-overlays-hidden = オーバーレイを非表示
measure-hint-length = 長さ: ドラッグするか両端をクリック
measure-hint-angle = 角度: 3 点をクリック (2 点目が頂点)
measure-hint-ellipse = 楕円: 領域をドラッグ
measure-hint-rectangle = 矩形: 領域をドラッグ
measure-off = 計測ツールをオフ
measure-cleared = 計測を消去しました
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = 面積 { $value } mm²
measure-area-px = 面積 { $value } px²
measure-angle = { $value }°
measure-mean = 平均 { $mean } ± { $sd } { $unit }
measure-range = 最小 { $min }  最大 { $max }  ({ $count } px)
measure-caveat-detector = 検出器上: 拡大率は未補正
measure-caveat-magnification = 推定の拡大率補正
measure-caveat-uncalibrated = ピクセル間隔なし: ピクセル単位
measure-caveat-outside-region = 校正済み超音波領域の外: ピクセル単位

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = 方向ラベルを表示
settings-medical-orientation-labels-help = 画像の端に表示される患者の方向を示す文字です（R/L は右/左、A/P は前/後、S/I は頭側/足側）。ファイルから取得され、回転や反転に追従します。L キーでセッション中の表示を切り替えます。
settings-medical-wheel-steps-frames = マウスホイールでフレームを切り替え
settings-medical-wheel-steps-frames-help = 複数のフレームを持つファイルやシリーズでは、ホイールで拡大・縮小や画像の切り替えではなくフレーム間を移動します。Ctrl+ホイールでは引き続き拡大・縮小します。
# machine-drafted, needs native review
settings-medical-display-function = グレー表示
settings-medical-display-function-help = 医用画像のグレーの表示方法です。DICOM GSDF（グレースケール標準表示関数）は、ディスプレイの黒と白の輝度の間で、見え方が等間隔になるようにグレーを割り当てます（DICOM PS3.14）。校正済みの診断用ディスプレイと同様の表示になります。ディスプレイのカラープロファイルは引き続き適用されます。
settings-medical-display-function-srgb = 標準（sRGB）
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = ディスプレイの黒輝度
settings-medical-display-black-help = 環境光を含めたディスプレイの黒の輝度です。測定値またはディスプレイの仕様書の値を使用します。DICOM GSDF で使用されます。
settings-medical-display-white = ディスプレイの白輝度
settings-medical-display-white-help = ディスプレイの白の輝度です。測定値またはディスプレイの仕様書の値を使用します。DICOM GSDF で使用されます。
palette-orientation-labels = 方向ラベルの表示/非表示
palette-presentation-state = 次のプレゼンテーションステート
disabled-no-presentation-state = この画像を参照するプレゼンテーションステートはありません
medical-labels-shown = 方向ラベルを表示
medical-labels-hidden = 方向ラベルを非表示
medical-presentation = プレゼンテーションステート: { $label } ({ $index } / { $count })
medical-presentation-none = プレゼンテーションステート オフ
medical-presentation-readout = プレゼンテーション: { $label }
medical-preset-presentation = プレゼンテーションステートのルックアップテーブルでグレーレベルを設定しています

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = 方向: 左 { $left }、右 { $right }、上 { $top }、下 { $bottom }
