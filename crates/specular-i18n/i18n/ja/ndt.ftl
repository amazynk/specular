# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = 検査
ndt-probe = プローブ: { $name }
ndt-probe-profile = ライン プロファイル
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = 複線形像質計 (IQI)
ndt-probe-wire-iqi = 線形像質計 (IQI)
ndt-probe-crack = ひび割れ幅
ndt-probe-density = 濃度ポイント

ndt-hint-profile = 線をドラッグすると、線に沿ったグレー値をグラフにします。[ と ] でプローブを切り替えます。
ndt-hint-snr = 平坦な部分で領域をドラッグします。領域は 20 × 55 ピクセル以上に広がります。
ndt-hint-duplex-iqi = 像質計に沿って、すべての線対を横切る線をドラッグします。
ndt-hint-wire-iqi = 見える中で最も細い線を横切る短い線をドラッグします。
ndt-hint-crack = ひび割れに沿ってクリックし、Enter を押します。幅は 10 か所で読み取ります。
ndt-hint-density = 点をクリックしてグレー値を読み取ります。既知の濃度を入力すると較正します。

ndt-refused-needs-scale = 先に縮尺を設定してください: 線径にはミリメートルが必要です。既知の長さで「較正」を使ってください。
ndt-refused-nothing-found = この線に沿ってノイズから際立つものがありません。
ndt-refused-too-small = 画像が 20 × 55 ピクセルの SNR 領域より小さいです。
ndt-refused-no-noise = この領域には測定できるノイズがありません (すべてのピクセルが同じ値です)。

ndt-reading = 読み取り中…
ndt-no-pixels = ピクセルを読み取れませんでした。

ndt-profile-range = グレー { $min } ～ { $max }
ndt-profile-mean = 平均グレー { $mean }
ndt-profile-density = 濃度 { $min } ～ { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = SNRn には SRb が必要です: 複線形像質計を測定するか、値を入力してください。
ndt-snr-stats = 平均 { $mean }、σ { $sd }、領域 { $w } × { $h } px

ndt-duplex-result = 最初の分離できない線対 { $pair }: SRb { $srb } µm、不鮮鋭度 { $unsharpness }
ndt-duplex-all-resolved = 見つかった線対はすべて分離しています。
ndt-duplex-dips = 谷: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = 補間値：iSRb { $isrb } µm、不鮮鋭度 { $unsharpness }

ndt-wire-result = 線番号 { $wire }、{ $diameter }
ndt-wire-measured = 測定幅 { $width } (FWHM)、直径約 { $diameter }

ndt-crack-mean = 平均幅 { $width }
ndt-crack-range = 最小 { $min }、最大 { $max }
ndt-crack-cuts = { $count } か所で測定

ndt-density-value = グレー { $grey }、濃度 { $density }
ndt-density-grey = グレー { $grey }
ndt-density-uncalibrated = 濃度はまだ較正されていません: 2 点で既知の濃度を入力してください。

ndt-ask-density = この点の既知の濃度: { $value } (Enter で適用、空のまま Enter でスキップ)
ndt-ask-srb = 基本空間分解能 SRb: { $value } µm (Enter で適用、Esc でスキップ)
ndt-bad-density = 0 ～ 10 の濃度を入力してください。
ndt-bad-srb = SRb をマイクロメートルで入力してください (例: 80)。
ndt-srb-set = この画像の SRb を { $srb } µm に設定しました。
ndt-density-one-more = 濃度ポイントを記録しました。もう 1 点で濃度を入力してください。
ndt-density-calibrated = 2 点から濃度を較正しました。
ndt-sidecar-failed = 検査設定をファイルの隣に保存できませんでした: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = 離陸地点から { $metres } m
map-export-kml = 写真を KMZ または KML で保存…
map-export-kmz-full = 写真をフルサイズの写真付き KMZ で保存…
map-kml-heading = 方位 { $degrees }°
map-kml-height = 離陸地点から { $metres } m
map-kml-nothing = 保存できる位置情報付きの写真がありません。
map-kml-saved = { $count } 枚の写真を { $name } に保存しました。
map-kml-failed = 地図ファイルを保存できませんでした: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = 点検報告書
inspection-report-default-title = 点検報告書
inspection-report-folder = このフォルダーの他の写真も追加
inspection-report-docx = Word 文書 (DOCX) も保存
inspection-report-kmz = 写真の撮影地点の KMZ も保存
inspection-report-caption = 計測付きの写真
inspection-report-col-value = 値
inspection-report-col-scale = 縮尺の基準
inspection-report-docx-failed = Word 文書を書き込めませんでした。
inspection-report-unreadable = どの写真も読み取れませんでした。
