# machine-drafted, needs native review
# 検査ツール：クリッピング警告、フォーカスピーキング、ヒストグラム、ピクセルインスペクター。

## コマンドパレット
palette-clipping = クリッピング警告の表示／非表示
palette-focus-peaking = フォーカスピーキングの表示／非表示
palette-histogram = ヒストグラムの表示／非表示
palette-pixel-inspector = ピクセルインスペクターの表示／非表示
# machine-drafted, needs native review
palette-crosshair = キーボード十字線
palette-announce-cursor = カーソル下の値を読み上げ

## ヒストグラム
histogram-title = ヒストグラム
histogram-clipped = クリッピング：ハイライト { $high }、シャドウ { $low }
histogram-empty = 表示中の画像がありません
histogram-a11y = 表示中の画像のヒストグラム：平均輝度 { $mean }、白飛び { $high }、黒つぶれ { $low }

## ピクセルインスペクター
inspector-title = ピクセルインスペクター
inspector-position = 位置
inspector-source-rgb = 元データ RGB
inspector-source-rgba = 元データ RGBA
inspector-source-ycc = 元データ Y′CbCr
inspector-source-gray = 元データ グレー
inspector-source-gray16 = 元データ グレー（16ビット）
inspector-source-gray-alpha16 = 元データ グレー + アルファ（16ビット）
inspector-source-rgb16 = 元データ RGB（16ビット）
inspector-source-rgba16 = 元データ RGBA（16ビット）
inspector-source-pending = 元データ
inspector-srgb = sRGB
inspector-hex = 16進
inspector-linear = リニア
inspector-approximate = 縮小プレビューの値です。正確な値は拡大してください
inspector-outside = 画像上にポインターを置くとピクセルを検査できます
inspector-a11y = 位置 { $x }, { $y } のピクセル：sRGB { $value }

## 設定
settings-section-inspection = 検査
settings-inspection-clip-highlight = ハイライトのクリッピングレベル
settings-inspection-clip-highlight-help = いずれかのチャンネルがこの8ビット値以上のピクセルを白飛びとして表示します。
settings-inspection-clip-shadow = シャドウのクリッピングレベル
settings-inspection-clip-shadow-help = いずれかのチャンネルがこの8ビット値以下のピクセルを黒つぶれとして表示します。
settings-inspection-highlight-color = ハイライト警告の色
settings-inspection-highlight-color-help = 白飛びを示す色です。
settings-inspection-shadow-color = シャドウ警告の色
settings-inspection-shadow-color-help = 黒つぶれを示す色です。
settings-inspection-peaking-sensitivity = フォーカスピーキングの感度
settings-inspection-peaking-sensitivity-help = 値を大きくすると、より柔らかいエッジもピントが合っていると表示します。
settings-inspection-peaking-color = フォーカスピーキングの色
settings-inspection-peaking-color-help = シャープなエッジを示す色です。
settings-inspection-histogram-channels = ヒストグラムのチャンネル
settings-inspection-histogram-channels-help = ヒストグラムに表示する曲線です。
settings-inspection-histogram-channels-colour = 赤・緑・青
settings-inspection-histogram-channels-luminance = 輝度
settings-inspection-histogram-channels-both = カラーと輝度
settings-inspection-histogram-log = 対数ヒストグラム
settings-inspection-histogram-log-help = 少ない値も見えるよう、ヒストグラムを対数で表示します。

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = 赤チャンネルを表示
palette-channel-green = 緑チャンネルを表示
palette-channel-blue = 青チャンネルを表示
palette-channel-alpha = アルファチャンネルを表示（もう一度押すと市松模様の上、次に単色の上）
palette-channel-luma = 輝度を表示
channel-normal = すべてのチャンネル
channel-red = 赤チャンネル
channel-green = 緑チャンネル
channel-blue = 青チャンネル
channel-alpha = アルファチャンネル
channel-alpha-checker = 市松模様の上の透明部分
channel-alpha-matte = 単色の上の透明部分
channel-luma = 輝度
disabled-no-alpha = この画像には透明部分がありません
settings-inspection-alpha-matte-color = アルファの背景色
settings-inspection-alpha-matte-color-help = チャンネル表示「単色の上の透明部分」で画像の背後に置く単色。

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = 疑似カラーの表示／非表示
palette-next-colour-map = 次のカラーマップ
palette-channel-colours = 次のチャンネル色 (赤–緑、マゼンタ–緑、マゼンタ–黄–シアン)
palette-hilo = HiLo の表示／非表示 (ビット深度の最大値と最小値の画素)
palette-colour-vision = 次の色覚シミュレーション
palette-colour-vision-off = 色覚シミュレーションを停止
palette-colour-vision-protanopia = 1 型色覚 (P 型) をシミュレート (赤錐体なし)
palette-colour-vision-deuteranopia = 2 型色覚 (D 型) をシミュレート (緑錐体なし)
palette-colour-vision-tritanopia = 3 型色覚 (T 型) をシミュレート (青錐体なし)
palette-colour-vision-achromatopsia = 全色盲をシミュレート (色なし)
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet (色覚に配慮なし)
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = 生の値
# Screen reader summary of the colour bar.
colour-bar-a11y = カラーバー { $map }: { $low } から { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = 疑似カラー: { $map }
badge-hilo = HiLo: 最大値と最小値の画素をマーク中
badge-colour-vision-protanopia = 1 型色覚をシミュレート中
badge-colour-vision-deuteranopia = 2 型色覚をシミュレート中
badge-colour-vision-tritanopia = 3 型色覚をシミュレート中
badge-colour-vision-achromatopsia = 全色盲をシミュレート中
channel-colours-red-green = チャンネル: 赤・緑・青
channel-colours-magenta-green = チャンネル: マゼンタ・緑
channel-colours-magenta-yellow-cyan = チャンネル: マゼンタ・黄・シアン
settings-inspection-false-colour-map = 疑似カラーのカラーマップ
settings-inspection-false-colour-map-help = 疑似カラーとそのカラーバーに使うカラーマップ。jet と phase 以外はどの色覚でも読み取れます。
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma (発散型)
settings-inspection-false-colour-map-vik = Vik (発散型)
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance (発散型)
settings-inspection-false-colour-map-curl = Curl (発散型)
settings-inspection-false-colour-map-phase = Phase (周期型)
settings-inspection-false-colour-map-jet = Jet (色覚に配慮なし)
settings-inspection-colour-bar = カラーバー
settings-inspection-colour-bar-help = 疑似カラーがオンの間、画像自身の単位で値を示すカラーバーを表示します。
settings-inspection-channel-colours = チャンネル色
settings-inspection-channel-colours-help = 色チャンネルの表示方法。マゼンタ–緑とマゼンタ–黄–シアンは赤緑色覚異常でも区別できます。
settings-inspection-channel-colours-red-green = 赤・緑・青
settings-inspection-channel-colours-magenta-green = マゼンタ・緑
settings-inspection-channel-colours-magenta-yellow-cyan = マゼンタ・黄・シアン
settings-inspection-colour-vision = 色覚シミュレーション
settings-inspection-colour-vision-help = ウィンドウ全体を色覚異常での見え方で表示します (Machado et al. 2009)。書き出しには影響しません。
settings-inspection-colour-vision-off = オフ
settings-inspection-colour-vision-protanopia = 1 型色覚 (P 型)
settings-inspection-colour-vision-deuteranopia = 2 型色覚 (D 型)
settings-inspection-colour-vision-tritanopia = 3 型色覚 (T 型)
settings-inspection-colour-vision-achromatopsia = 全色盲
settings-inspection-auto-range = 高ビット深度グレースケールの自動レンジ
settings-inspection-auto-range-help = 16 ビットのグレースケール画像 (TIFF、PNG、FITS) を最も暗い値から最も明るい値まで引き伸ばします。DICOM 画像は独自のウィンドウを保ちます。

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = 十字線オン。矢印キーで移動、Shift で大きく移動、Enter でクリック、Esc で終了します。
crosshair-off = 十字線オフ
announce-cursor-nothing = カーソル下には何もありません
