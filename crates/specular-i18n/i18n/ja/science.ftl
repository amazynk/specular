# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. 日本語.

keys-mode-science = 科学画像
cheatsheet-science = 科学画像
cheatsheet-science-look = ストレッチと表示範囲
cheatsheet-science-planes = プレーンとチャンネル
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = チャンネル 1〜9 の表示/非表示
cheatsheet-science-channels-10-16 = チャンネル 10〜16 の表示/非表示
palette-science-stretch = 次のストレッチ
palette-science-limits = 次の表示範囲
palette-science-next-plane = 次のプレーン
palette-science-prev-plane = 前のプレーン
palette-science-next-axis = 別の軸を送る
palette-science-composite = チャンネルをカラー合成
palette-science-channel-1 = チャンネル 1 の表示/非表示
palette-science-channel-2 = チャンネル 2 の表示/非表示
palette-science-channel-3 = チャンネル 3 の表示/非表示
palette-science-channel-4 = チャンネル 4 の表示/非表示
palette-science-channel-5 = チャンネル 5 の表示/非表示
palette-science-channel-6 = チャンネル 6 の表示/非表示
palette-science-channel-7 = チャンネル 7 の表示/非表示
palette-science-channel-8 = チャンネル 8 の表示/非表示
palette-science-channel-9 = チャンネル 9 の表示/非表示
# machine-drafted, needs native review
palette-science-channel-10 = チャンネル 10 の表示/非表示
palette-science-channel-11 = チャンネル 11 の表示/非表示
palette-science-channel-12 = チャンネル 12 の表示/非表示
palette-science-channel-13 = チャンネル 13 の表示/非表示
palette-science-channel-14 = チャンネル 14 の表示/非表示
palette-science-channel-15 = チャンネル 15 の表示/非表示
palette-science-channel-16 = チャンネル 16 の表示/非表示
palette-science-grid = 座標グリッドの表示/非表示
palette-science-reset = 自動表示
palette-science-level = 高さマップを平面補正 (平均平面を減算)
palette-science-wiggle = 地震波形のウィグル表示 (バリアブルエリア付き) を色の上に重ねる
# machine-drafted, needs native review
palette-science-wireframe = 非構造メッシュの辺 (ワイヤーフレーム) を色の上に重ねる
palette-science-import-lut = ファイルからカラーテーブル (ImageJ .lut)…
palette-science-hillshade = 標高モデルの起伏を陰影表示 (陰影起伏)
palette-science-fft = 面のパワースペクトル (FFT)
palette-science-catalogue = 天体カタログを重ねる (VOTable、FITS テーブル、CSV)…
palette-science-clear-catalogues = 重ねたカタログを削除
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }: { $count ->
       *[other] 画像上の天体 { $count } 個
    }
science-catalogue-capped = { $name }: 画像上の天体 { $count } 個のうち { $shown } 個を表示
science-catalogue-none = { $name }: 画像上に天体はありません
science-catalogue-failed = 天球位置を持つカタログではありません: { $name }
science-catalogues-cleared = カタログを削除しました
# The file dialog's filter name for catalogue files.
science-catalogue-filter = 天体カタログ
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = m/z を指定してイオンイメージ (質量分析)…
disabled-not-science = 科学画像 (天文、電子顕微鏡、走査型プローブ顕微鏡) のみ
disabled-one-plane = 画像のプレーンは 1 つです
disabled-one-axis = 画像にはほかに送れる軸がありません
disabled-no-channels = 画像には合成できるそのチャンネルがありません
disabled-no-coordinates = 画像に天球座標がありません
disabled-not-height-map = 高さマップ (Gwyddion、Igor) のみ
disabled-not-elevation = 1 バンドのジオリファレンス済み標高モデルのみ
disabled-not-seismic = 地震探査データ (SEG-Y、Seismic Unix) のみ
disabled-not-mass-spec = 質量分析イメージ (imzML) のみ
# machine-drafted, needs native review
disabled-not-ugrid = 非構造メッシュ (UGRID・ICON・MPAS の NetCDF) のみ
disabled-not-bayer = カラーカメラの RAW フレームのみ (ベイヤー配列付き FITS、SER 動画)
palette-debayer = カラーカメラのフレームをデベイヤーして、または RAW のまま表示
palette-bayer-pattern = このファイルの次のベイヤー配列
science-debayer-on = デベイヤー済み ({ $pattern })
science-debayer-off = RAW カラーフィルター配列
science-bayer-pattern = ベイヤー配列 { $pattern }
# machine-drafted, needs native review
palette-annotations = 注釈の表示/非表示
disabled-no-annotations = 画像の横に注釈ファイルがありません (NDPA、Aperio、ASAP XML)
science-annotations-on = 注釈を表示
science-annotations-off = 注釈を非表示
science-stretch-auto = 自動 (色ごと)
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = リニア
science-stretch-sqrt = 平方根
science-stretch-log = 対数
science-stretch-asinh = Asinh
science-stretch-histeq = ヒストグラム平坦化
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = 最小/最大
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = ファイルの表示範囲
# Toasts when the look changes.
science-stretch-toast = ストレッチ: { $stretch }
science-limits-toast = 表示範囲: { $limits }
science-reset-toast = 自動表示
# An imported colour table: $name is its file name.
science-lut-toast = カラーテーブル { $name }
science-lut-failed = カラーテーブルではありません: { $name }
# The file dialog's filter name for .lut files.
science-lut-filter = ImageJ カラーテーブル
science-composite-on = チャンネルをカラー合成しました
science-composite-off = 1 チャンネルをグレーで表示
science-channel-on = { $channel } を表示
science-channel-off = { $channel } を非表示
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = チャンネル { $number }
science-grid-on = 座標グリッドを表示
science-grid-off = 座標グリッドを非表示
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = 分解能リングを表示
science-rings-off = 分解能リングを非表示
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = ウィグル表示オン (拡大すると表示)
science-wiggle-off = ウィグル表示オフ
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = メッシュの辺オン
science-wireframe-off = メッシュの辺オフ
science-wireframe-dense = メッシュの辺オン (拡大すると表示)
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = 平面補正オン
science-level-off = 平面補正オフ
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = 陰影起伏オン: 太陽の方位 { $azimuth }°、高度 { $altitude }°
science-hillshade-off = 陰影起伏オフ
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = パワースペクトルを表示
science-fft-off = パワースペクトルを非表示
science-axis-toast = 送り軸: { $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = 値 { $value }
science-value-unit = 値 { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = 値 { $value }（ビット { $bits }）
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = 値 { $value }・{ $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = データなし
s2-class-defective = 飽和または欠陥
s2-class-dark = 暗部ピクセル
s2-class-cloud-shadow = 雲の影
s2-class-vegetation = 植生
s2-class-not-vegetated = 非植生
s2-class-water = 水域
s2-class-unclassified = 未分類
s2-class-cloud-medium = 雲（中確率）
s2-class-cloud-high = 雲（高確率）
s2-class-cirrus = 薄い巻雲
s2-class-snow = 雪または氷
s2-class-clear = 晴天
s2-class-opaque-cloud = 不透明な雲
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = 赤経 { $ra }  赤緯 { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = WCS 非対応: { $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = カタログのマーカー: ICRS → FK4 に変換
science-fk4-markers-galactic = カタログのマーカー: 銀河座標 → FK4 に変換
science-fk4-samp = SAMP: FK4 → ICRS に変換
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = 平面補正済み
# Shown under the look while the hillshade is on.
science-hillshaded = 陰影起伏: 太陽の方位 { $azimuth }°、高度 { $altitude }°
# Shown in the readout while the power spectrum is shown.
science-fft = パワースペクトル (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ パワー { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = 周波数 u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = 周期 { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = 相 { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = { $x }, { $y } のスペクトル
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range } (ビン { $bin })
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = アンダーフロー/オーバーフローのビン { $count } 個を非表示 (合計 { $sum })
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = 可変幅ビンを等間隔グリッドに再サンプリング
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
   *[other] { $count } イベント
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = 補正済み ({ $keyword })
science-cytometry-uncompensated = 未補正
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = インライン { $inline }  クロスライン { $crossline }
science-seismic-trace = トレース { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = トレースなし
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = 時間 { $time } ms
science-seismic-depth = 深度 { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = 面 { $face }
science-mesh-face-node = 面 { $face }  節点 { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = メッシュ外
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index } / { $count }
science-plane-world = { $axis } { $index } / { $count } ({ $world })
science-axis-unnamed = プレーン
science-axis-extension = 画像
science-axis-channel = チャンネル
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = フィールド
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = ビン
science-bin-size = { $size } px
science-axis-band = エネルギー
science-band-all = 全エネルギー
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = 成分
science-part-magnitude = 絶対値
science-part-real = 実部
science-part-imaginary = 虚部
science-part-phase = 位相 (ラジアン)
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = 強度 (dB)
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = フレーム
science-readout = 値と座標
# Accessible names of the plane sliders.
science-sliders = プレーン
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = スケールバー: { $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = 全イオン電流
science-ion-window = イオンイメージ m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = イオンイメージの m/z: { $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab 許容誤差 · Enter 表示 · Esc キャンセル
science-ion-bad = m/z ではありません: { $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = スポット x { $x }  y { $y }
science-ms-spot-z = スポット x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = このスポットにはスペクトルがありません
# machine-drafted, needs native review
palette-science-temperature-unit = 温度単位 (°C、°F、K)
palette-science-isotherm = ポインタ位置の温度より上または下の等温線
palette-science-thermal-parameters = 放射率と反射温度…
palette-science-export-temperatures = 温度を書き出す (浮動小数点 TIFF または CSV)…
palette-thermal-visible = 熱画像の可視光写真
disabled-not-thermal = 熱画像のみ
disabled-not-radiometric = このファイルは温度を保存しています: パラメータは変更できません
disabled-no-visible-photo = この熱画像には可視光写真がありません
science-temperature-unit-toast = 温度の単位: { $unit }
science-isotherm-above = { $level } 以上の等温線
science-isotherm-below = { $level } 以下の等温線
science-isotherm-off = 等温線オフ
science-thermal-emissivity-prompt = 放射率: { $text }
science-thermal-reflected-prompt = 反射温度 ({ $unit }): { $text }
science-thermal-prompt-hint = Tab 次のフィールド · Enter 再計算 · Esc キャンセル
science-thermal-bad = 無効な値です: { $text }
science-thermal-applied = 放射率 { $emissivity }、反射温度 { $reflected } で再計算しました
science-thermal-conditions = 放射率 { $emissivity } · 反射温度 { $reflected }
science-thermal-export-tiff = 32 ビット浮動小数点 TIFF
science-thermal-export-csv = CSV
science-thermal-export-done = 温度を { $name } に保存しました
science-thermal-export-failed = 温度を保存できませんでした: { $reason }
# machine-drafted, needs native review
science-values-failed = 値を表示できませんでした: { $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = 熱画像シーケンスを再生または一時停止
palette-science-thermal-series-csv = 温度の時間変化を保存 (CSV)…
disabled-not-thermal-sequence = 熱画像シーケンスのみ
thermal-seq-title = 温度の時間変化
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = フレーム { $frame }/{ $frames } · { $time }
thermal-seq-play = 再生
thermal-seq-pause = 一時停止
thermal-seq-playing = 再生中
thermal-seq-paused = 一時停止中
thermal-seq-save-csv = CSV を保存…
thermal-seq-hint = スポット、ライン、エリア (M) を追加すると、時間変化をグラフにします。
thermal-seq-reading = フレーム { $done }/{ $total } を読み込み中…
thermal-seq-failed = フレームを読み込めませんでした: { $reason }
thermal-seq-no-series = まだ保存するものがありません。スポット、ライン、エリアを追加してください。
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = グラフ: { $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = レポート (PDF)…
thermal-report-heading = 熱画像レポート
thermal-report-default-title = 赤外線サーモグラフィ点検報告書
thermal-report-title-label = タイトル
thermal-report-notes-label = メモ
thermal-report-photo = 可視光写真を含める
thermal-report-folder = このフォルダーの他の熱画像も追加
thermal-report-save = PDF を保存…
thermal-report-busy = レポートを作成中…
thermal-report-nothing = 温度はまだ表示されていません。
thermal-report-saved = レポートを { $name } に保存しました ({ $count ->
   *[other] { $count } 枚
})
thermal-report-failed = レポートを保存できませんでした: { $reason }
# In the PDF.
thermal-report-notes = メモ
thermal-report-measurements = 測定
thermal-report-no-measurements = 測定はありません。
thermal-report-thermal-caption = 熱画像
thermal-report-visible-caption = 可視光写真
thermal-report-col-measurement = 測定
thermal-report-col-min = 最小
thermal-report-col-max = 最大
thermal-report-col-mean = 平均
thermal-report-col-emissivity = 放射率
thermal-report-whole-image = 画像全体
thermal-report-camera = カメラ
thermal-report-lens = レンズ
thermal-report-date = 撮影日時
thermal-report-place = 撮影地
thermal-report-frame = フレーム
thermal-report-emissivity = 放射率
thermal-report-reflected = 反射温度
thermal-report-distance = 距離
thermal-report-humidity = 湿度
thermal-report-range = 範囲
thermal-report-format = 形式
thermal-report-page = { $page }/{ $pages } ページ
