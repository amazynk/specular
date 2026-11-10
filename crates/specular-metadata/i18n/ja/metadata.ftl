# machine-drafted, needs native review
# Specular メタデータパネル — 日本語（規約は i18n/en/metadata.ftl を参照。数値と日付は ICU4X で書式化済み）

## 概要のグループ名

group-camera = カメラ
group-exposure = 露出
group-lens = レンズ
group-date = 日付と時刻
group-location = 撮影地
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = ドローン
label-gimbal = ジンバル
label-flight = 機体
label-relative-altitude = 離陸地点からの高度
drone-attitude = ヨー { $yaw }、ピッチ { $pitch }、ロール { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = 温度
drone-thermal-refused = 表示できません: DJI の SDK ライセンスで認められていません
drone-thermal-refused-detail = DJI の温度を読むには DJI Thermal SDK が必要ですが、そのライセンスでは使えるのはそれを同梱するプログラムだけです。画像とメタデータは表示されます。
group-image = 画像
group-file = ファイル
group-rights = 権利と説明
group-credentials = コンテンツクレデンシャル
# machine-drafted, needs native review
group-medical = 医用画像
# machine-drafted, needs native review
group-inspection = 部品と検査
group-nde-image = 検査画像 (DICONDE)
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = 検査画像
group-science = 科学画像
group-em = 電子顕微鏡
group-spm = 走査型プローブ顕微鏡
group-slide = スライド
group-microscopy = 顕微鏡
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = 回折
group-surface = 表面形状
group-ebsd = 電子線後方散乱回折
group-seismic = 地震探査データ
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = CASA 画像
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = トラクトグラフィー
# machine-drafted, needs native review
group-annotations = 注釈
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = 質量分析イメージング
# machine-drafted, needs native review
group-thermal = 熱画像
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = フローサイトメトリー
# machine-drafted, needs native review
group-spectroscopy = 分光
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = 非構造メッシュ
group-drawing = 図面
# machine-drafted, needs native review
group-volume = ボリューム

## 概要の項目名

label-camera = カメラ
label-software = ソフトウェア
label-application = アプリケーション
label-shutter = シャッタースピード
label-aperture = 絞り値
label-iso = ISO感度
label-focal-length = 焦点距離
label-exposure-bias = 露出補正
label-program = 撮影モード
label-metering = 測光モード
label-white-balance = ホワイトバランス
label-flash = フラッシュ
label-lens = レンズ
label-lens-make = レンズメーカー
label-lens-range = レンズ仕様
# machine-drafted, needs native review
label-focus-mode = フォーカスモード
label-stabilisation = 手ブレ補正
label-taken = 撮影日時
label-digitised = デジタル化日時
label-edited = 最終編集日時
label-place = 場所
label-coordinates = 座標
label-altitude = 高度
label-direction = 撮影方向
label-accuracy = 測位精度
label-gps-time = GPS時刻
label-dimensions = サイズ
label-encoding = エンコード
# machine-drafted, needs native review
label-motion = モーション
motion-kind-motion-photo = モーションフォト
motion-kind-micro-video = モーションフォト (MicroVideo)
motion-kind-samsung = Samsung モーションフォト
motion-kind-live-photo = Live Photo
motion-duration = { $seconds } 秒
label-colour-profile = カラープロファイル
label-orientation = 向き
label-file-name = 名前
label-folder = フォルダー
label-file-size = ファイルサイズ
label-format = 形式
label-modified = 更新日時
label-title = タイトル
label-description = 説明
label-creator = 作成者
label-copyright = 著作権
label-keywords = キーワード
label-rating = レーティング
label-colour-label = カラーラベル
label-credentials = コンテンツクレデンシャル

## 「すべてのフィールド」のグループ名（タグ名は規格のまま）

fields-ifd0 = IFD0（メイン画像）
fields-exif = Exif
fields-gps = GPS
fields-interop = 互換性（Interoperability）
fields-ifd1 = IFD1（サムネイル）
fields-icc = ICCプロファイルヘッダー
fields-xmp = XMP：{ $namespace }（{ $prefix }）
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] 基本
    [xmpRights] 権利
    [xmpMM] メディア管理
    [xmpDM] ダイナミックメディア
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] カメラアクセサリー
    [photoshop] Photoshop
    [crs] Camera Raw 設定
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] PLUS ライセンス
    [GPano] パノラマ（Photo Sphere）
    [GCamera] Google カメラ
    [hdrgm] HDR ゲインマップ
    [mwg-rs] 領域
    [pdf] PDF
    [stEvt] リソースイベント
    [stRef] リソース参照
   *[other] その他
}
unknown-tag = 不明なタグ { $number }（{ $context }）
blob-bytes = { $num ->
   *[other] （{ $num } バイト）
}
maker-note-bytes = （{ $num } バイト、メーカーノート、未解析）
more-values = { $shown }、…（全 { $total } 個の値）

## 区切り文字

list-separator = {"、"}
one-line-separator = {" · "}

## 露出

exposure-time-fraction = 1/{ $denominator } 秒
exposure-time-seconds = { $seconds } 秒
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm（35mm判換算 { $equiv } mm）
focal-length-equiv-only = 35mm判換算 { $equiv } mm
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] マニュアル
    [2] プログラムAE
    [3] 絞り優先AE
    [4] シャッター優先AE
    [5] クリエイティブ（被写界深度優先）
    [6] アクション（高速シャッター優先）
    [7] ポートレート
    [8] 風景
   *[other] 不明
}
metering-mode = { $value ->
    [1] 平均測光
    [2] 中央重点平均測光
    [3] スポット測光
    [4] マルチスポット測光
    [5] 評価測光（マルチパターン）
    [6] 部分測光
   *[other] その他
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] ワンショットAF
    [1] AIサーボAF
    [2] AIフォーカスAF
    [3] マニュアルフォーカス
    [4] シングル
    [5] コンティニュアス
    [6] マニュアルフォーカス
    [16] パンフォーカス
    [256] ワンショットAF（ライブビュー）
    [257] AIサーボAF（ライブビュー）
    [258] AIフォーカスAF（ライブビュー）
    [512] ムービースナップフォーカス
    [519] 動画サーボAF
   *[other] その他
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] 切
    [1] 入
    [2] 撮影時のみ
    [3] 流し撮り
    [4] ダイナミック
    [256] 切
    [257] 入
    [258] 撮影時のみ
    [259] 流し撮り
    [260] ダイナミック
   *[other] その他
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] オート
    [2] マニュアルフォーカス
    [4] オート（フォーカスボタン）
    [5] オート（コンティニュアス）
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] その他
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] オン（光学式）
    [3] オフ
    [4] オン（モード 2）
    [5] オン（光学式、流し撮り）
    [6] オン（ボディのみ）
    [7] オン（ボディのみ、流し撮り）
    [9] Dual IS
    [10] Dual IS（流し撮り）
    [11] Dual IS 2
    [12] Dual IS 2（流し撮り）
   *[other] その他
}
label-comment = コメント
fields-tags = { $kind } タグ
group-picture = 画像設定
fields-fujifilm = 富士フイルムのメーカーノート
label-film-simulation = フィルムシミュレーション
label-dynamic-range = ダイナミックレンジ
label-highlight-tone = ハイライト
label-shadow-tone = シャドウ
label-colour-setting = カラー
label-sharpness = シャープネス
label-noise-reduction = ノイズリダクション
label-clarity = 明瞭度
label-grain-effect = グレイン・エフェクト
label-colour-chrome = カラークローム・エフェクト
label-colour-chrome-blue = カラークローム・ブルー
label-shutter-type = シャッター方式
label-drive-mode = ドライブモード
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia（スタンダード）
    [256] スタジオポートレート
    [272] スタジオポートレート（彩度強調）
    [288] Astia（ソフト）
    [304] スタジオポートレート（シャープ）
    [512] Velvia（ビビッド）
    [768] スタジオポートレート Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] その他
}
fuji-monochrome = { $value ->
    [768] モノクロ
    [769] モノクロ、レッドフィルター
    [770] モノクロ、イエローフィルター
    [771] モノクロ、グリーンフィルター
    [784] セピア
    [1280] Acros
    [1281] Acros、レッドフィルター
    [1282] Acros、イエローフィルター
    [1283] Acros、グリーンフィルター
   *[other] モノクロ
}
fuji-white-balance = { $value ->
    [0] オート
    [1] オート（ホワイト優先）
    [2] オート（雰囲気優先）
    [256] 晴天
    [512] 日陰
    [768] 昼光色蛍光灯
    [769] 昼白色蛍光灯
    [770] 白色蛍光灯
    [771] 温白色蛍光灯
    [772] 電球色蛍光灯
    [1024] 電球
    [1280] フラッシュ
    [1536] 水中
    [3840] カスタム 1
    [3841] カスタム 2
    [3842] カスタム 3
    [3843] カスタム 4
    [3844] カスタム 5
    [4080] 色温度
   *[other] その他
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = オート（DR{ $percent }）
fuji-dynamic-range-setting = { $value ->
    [0] オート
    [256] スタンダード（DR100）
    [512] ワイド 1（DR230）
    [513] ワイド 2（DR400）
   *[other] その他
}
fuji-effect = { $value ->
    [0] オフ
    [32] 弱
    [64] 強
   *[other] その他
}
fuji-grain-size = { $value ->
    [16] 小
    [32] 大
   *[other] その他
}
fuji-grain = { $strength }、{ $size }
fuji-shutter-type = { $value ->
    [0] メカニカル
    [1] 電子
    [2] 電子（長秒時）
    [3] 電子先幕
   *[other] その他
}
fuji-drive-mode = { $value ->
    [0] 単写
    [1] 連写（低速）
    [2] 連写（高速）
   *[other] その他
}
fuji-focus-mode = { $value ->
    [0] マニュアル
    [1] AF-S
    [2] AF-C
    [100] オート
    [101] マニュアル
   *[other] その他
}
fuji-stabilisation = { $kind }（{ $mode }）
fuji-stabilisation-kind = { $value ->
    [0] なし
    [1] 光学式
    [2] センサーシフト
    [3] レンズ
    [258] センサーシフトとデジタル
    [512] デジタル
   *[other] その他
}
fuji-stabilisation-mode = { $value ->
    [0] オフ
    [1] オン、常時
    [2] オン、撮影時のみ
   *[other] その他
}
light-source = { $value ->
    [1] 太陽光
    [2] 蛍光灯
    [3] 白熱電球（タングステン）
    [4] フラッシュ
    [9] 晴天
    [10] 曇天
    [11] 日陰
    [12] 昼光色蛍光灯
    [13] 昼白色蛍光灯
    [14] 白色蛍光灯
    [15] 温白色蛍光灯
    [16] 電球色蛍光灯
    [17] 標準光A
    [18] 標準光B
    [19] 標準光C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] スタジオ用タングステン
   *[other] その他
}
white-balance-auto = オート
white-balance-manual = マニュアル
white-balance-manual-with-light = マニュアル（{ $light }）
flash-none = フラッシュ機能なし
flash-fired = フラッシュ発光
flash-not-fired = フラッシュ非発光
flash-forced-on = 強制発光
flash-forced-off = 発光禁止
flash-auto = オートモード
flash-return-not-detected = 反射光未検出
flash-return-detected = 反射光検出
flash-red-eye = 赤目軽減

## レンズ

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## 日付と時刻

date-with-weekday = { $date }（{ $weekday }）
date-time = { $date } { $time }
date-time-zone = { $datetime }（{ $zone }）
zone-utc = UTC
zone-offset = UTC{ $offset }
gps-date-time = { $date } { $time } UTC
detail-zone-from-gps = タイムゾーンは GPS 時刻から算出しました

## 撮影地

coordinates = { $latRef ->
    [south] 南緯
   *[north] 北緯
} { $lat }°、{ $lonRef ->
    [west] 西経
   *[east] 東経
} { $lon }°
altitude = { $ref ->
    [above] 海抜 { $m } m
    [below] 海面下 { $m } m
   *[sea] 海抜 0 m
}
compass-point = { $point ->
    [n] 北
    [nne] 北北東
    [ne] 北東
    [ene] 東北東
    [e] 東
    [ese] 東南東
    [se] 南東
    [sse] 南南東
    [s] 南
    [ssw] 南南西
    [sw] 南西
    [wsw] 西南西
    [w] 西
    [wnw] 西北西
    [nw] 北西
   *[nnw] 北北西
}
facing = { $point }向き（{ $degrees }°）
detail-true-north = 真北基準
detail-magnetic-north = 磁北基準
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $country }、{ $city }
place-city-only = { $city }
place-near = { $place }付近
place-detail = { $areas } · { $city }から{ $distance }
place-detail-distance = { $city }から{ $distance }

## 画像

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height }（{ $mp } MP）
encoding-bit-depth = { $bits } ビット
encoding-progressive = { $format }、プログレッシブ
encoding-baseline = { $format }、ベースライン
encoding-hierarchical = { $format }、階層型
# machine-drafted, needs native review
encoding-alpha = { $format }、{ $alpha ->
    [premultiplied] 乗算済みアルファ付き
    [transparency] 透過あり
   *[alpha] アルファ付き
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] グレースケール
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] インデックスカラー
    [Multichannel] マルチチャンネル
    [Duotone] ダブルトーン
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] 輝度/色差 (YC)
    [Bayer] Bayer RAW
    [X-Trans] X-Trans RAW
    [CFA] カラーフィルター配列 RAW
    [LinearRaw] リニア RAW
    [RGBE] RGBE (共有指数)
    [XYZE] XYZE (共有指数)
    [R] 単一チャンネル (R)
    [RG] 2 チャンネル (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB（EXIF タグによる。埋め込みプロファイルなし）
orientation = { $value ->
    [1] 正位置（回転不要）
    [2] 左右反転（適用済み）
    [3] 180° 回転（適用済み）
    [4] 上下反転（適用済み）
    [5] 左右反転して反時計回りに 90° 回転（適用済み）
    [6] 時計回りに 90° 回転（適用済み）
    [7] 左右反転して時計回りに 90° 回転（適用済み）
    [8] 反時計回りに 90° 回転（適用済み）
   *[other] 不明（{ $value }）
}
orientation-short = { $value ->
    [1] 正位置
    [2] 左右反転
    [3] 180° 回転
    [4] 上下反転
    [5] 左右反転して反時計回りに 90° 回転
    [6] 時計回りに 90° 回転
    [7] 左右反転して時計回りに 90° 回転
    [8] 反時計回りに 90° 回転
   *[other] 値 { $value }
}

## ファイル

file-size-bytes = { $num ->
   *[other] { $num } バイト
}
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## 権利

rating-rejected = 除外
rating-stars = { $stars }（5 段階中 { $n }）

## コンテンツクレデンシャル

c2pa-present = あり（未検証）
c2pa-detail = このファイルには C2PA マニフェストが含まれていますが、署名は検証されていません。

## ICC ヘッダーの値

icc-class = { $class ->
    [scnr] 入力デバイス
    [mntr] ディスプレイデバイス
    [prtr] 出力デバイス
    [link] デバイスリンク
    [spac] 色空間
    [abst] 抽象
    [nmcl] 名前付きカラー
   *[other] 不明
}
icc-intent = { $intent ->
    [0] 知覚的
    [1] 相対的な色域を維持
    [2] 彩度
    [3] 絶対的な色域を維持
   *[other] 不明
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] 指定なし
   *[other] 不明
}
icc-coded = { $name }（{ $code }）

# machine-drafted, needs native review
label-modality = モダリティ
label-body-part = 部位
label-study = 検査
label-series = シリーズ
label-slice-thickness = スライス厚
label-pixel-spacing = ピクセル間隔
label-imager-pixel-spacing = 検出器上のピクセル間隔
label-frames = フレーム
label-transfer-syntax = 転送構文
label-document = ドキュメント
label-document-type = ドキュメントの種類
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (非可逆圧縮)
detail-pixel-spacing = 患者内でのピクセル中心間の距離（行 × 列）。
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = 対象物内でのピクセル中心間の距離（行 × 列）。
detail-imager-pixel-spacing = 検出器で測定した値で、拡大率は補正されていません。患者内の距離は通常これより小さくなります。

## 工業検査 (DICONDE)

# machine-drafted, needs native review
label-nde-component-shape = 部品形状
label-nde-component-manufacturer = 部品メーカー
label-nde-material = 材質
label-nde-thickness = 材料厚さ
label-nde-pipe-diameter = 管径
label-nde-source = 線源
label-nde-environment = 環境
label-nde-iqi = 透過度計
label-nde-indications = 指示
label-nde-standard = 規格

## 研究用ボリューム (NIfTI、Analyze、NRRD、MetaImage)

# machine-drafted, needs native review
label-voxel-size = ボクセルサイズ
label-data-type = データ型
label-time-points = 時点数
label-orientation-source = 方向の取得元
label-intent = 値の意味
label-compression = 圧縮
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Analyze ヘッダー (信頼性なし)
volume-orient-space = NRRD 空間方向
volume-orient-transform = MetaImage 変換
volume-orient-spacings = ボクセル間隔のみ
volume-orient-none = 記載なし
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = FreeSurfer 既定値（信頼性なし）
volume-orient-minc = MINC 次元方向余弦
volume-orient-object = 物体座標 (患者の向きなし)
volume-content-projections = 投影 (生スキャン)
volume-content-reconstruction = 再構成ボリューム
volume-coords-rectilinear = 直線格子 (不等間隔を等間隔で表示)
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = 繰り返し時間
label-volume-te = エコー時間
label-volume-ti = 反転時間
label-volume-flip = フリップ角
label-volume-fov = 撮像視野
label-volume-field = 静磁場強度
label-volume-pedir = 位相エンコード
label-volume-cmdline = コマンドライン
label-volume-xform = Talairach 変換
label-volume-content = 内容
label-volume-projections = 投影数
label-volume-angles = 回転範囲
label-volume-angle-step = 回転ステップ
label-volume-kv = 管電圧
label-volume-current = 管電流
label-volume-power = 線源出力
label-volume-exposure = 露光
label-volume-sod = 線源–物体間距離
label-volume-odd = 物体–検出器間距離
label-volume-sdd = 線源–検出器間距離
label-volume-magnification = 光学倍率
label-volume-binning = ビニング
label-volume-objective = 対物レンズ
label-volume-filter = 線源フィルター
label-volume-camera = 検出器
label-volume-instrument = 装置
label-volume-software = ソフトウェア
label-volume-date = 取得日時
label-volume-ring = リングアーチファクト補正
label-volume-smoothing = 平滑化
label-volume-coords = グリッド
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = 眼
volume-eye-right = 右眼 (OD)
volume-eye-left = 左眼 (OS)
detail-volume-unreliable = このファイルには、軸が患者のどの方向を向くかが記載されていません。左右が入れ替わっている可能性があるため、方向ラベルは表示しません。
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = 軸は患者ではなくスキャナーのものであるため、方向ラベルは表示しません。

## 科学画像 (FITS、XISF)

label-object = 天体
label-telescope = 望遠鏡
label-instrument = 観測装置
label-filter = フィルター
label-observed = 観測日時
label-exposure-time = 露出時間
label-sample = サンプル
label-unit = 単位
label-hdu = 拡張
label-centre = 中心
label-centre-deg = 中心 (度)
label-frame = 座標系
# machine-drafted, needs native review
science-frame-galactic = 銀河座標
science-frame-ecliptic = 黄道座標
label-projection = 投影法
label-pixel-scale = ピクセルスケール
science-seconds = { $seconds } 秒
science-arcsec-px = 1 ピクセルあたり { $arcsec }″
label-distortion = 歪み補正
label-wcs = 天球座標
science-wcs-unsupported = 非対応: { $reason }
# machine-drafted, needs native review
science-wcs-approximate = 近似（指向のみ、歪み補正なし）
label-observer = 観測者
label-bayer = ベイヤー配列
label-capture-start = 撮影開始 (UTC)
label-capture-end = 撮影終了 (UTC)
label-frame-rate = フレームレート
science-fps = { $fps } fps
science-frames-cut = { $frames } / { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = データセット
label-pixel-size = ピクセルサイズ
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = 1 ピクセルあたり { $size } { $unit }
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = 顕微鏡
label-voltage = 加速電圧
label-magnification = 倍率
label-imaging-mode = モード
label-detector = 検出器
label-acquired = 取得日時
label-field-of-view = 視野
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = メーカー
label-scanner = スキャナー
label-objective = 対物レンズ
label-pyramid-levels = レベル
# machine-drafted, needs native review
label-emissivity = 放射率
label-object-distance = 対象物までの距離
label-reflected-temperature = 反射温度
label-atmospheric-temperature = 大気温度
label-humidity = 相対湿度
label-raw-range = 生データの範囲
label-temperature-range = 温度範囲
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }: { $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = 波長
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = 光子エネルギー
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = 検出器距離
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = ビーム中心
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = 検出器 2θ
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = ファイ (φ)
label-omega = オメガ (ω)
label-chi = カイ (χ)
label-kappa = カッパ (κ)
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = 振動
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = { $axis } 軸まわり { $start }° + { $width }°
# The count at which a pixel saturates.
label-overload = オーバーロード値
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = 分解能
xtal-resolution = 端で { $edge } Å、隅で { $corner } Å
label-geometry = ジオメトリ
xtal-curved = 湾曲したイメージングプレート: q、d、分解能リングなし
xtal-incomplete = ヘッダーが不完全: q、d、分解能リングなし
# All-fields group title: "FITS header".
fields-science-header = { $format } ヘッダー
# machine-drafted, needs native review
fields-asdf-tree = ASDF ツリー
label-asdf-model = データモデル
label-asdf-version = ファイルバージョン
label-asdf-standard = ASDF 標準
label-asdf-library = 書き込みソフト
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = ROOT キー
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = ROOT バージョン
label-root-compression = 圧縮
label-root-keys = キー
label-root-class = クラス
label-root-name = 名前
label-root-title = タイトル
label-root-entries = エントリ数
label-root-sum-weights = 重みの合計
# A graph's number of points.
label-root-points = 点数
# $axis is "x", "y" or "z".
label-root-axis = { $axis } 軸
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }: { $bins } ビン、{ $min } ～ { $max }
root-axis-variable = { $quantity }: 可変幅 { $bins } ビン、{ $min } ～ { $max }
label-root-mean-rms = 平均と RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = ツリー
root-tree = { $name } ({ $entries } エントリ)
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = 画像の種類
casa-type-image = 復元画像
casa-type-residual = 残差
casa-type-model = モデル
casa-type-pb = 主ビーム
casa-type-mask = クリーンマスク
casa-type-psf = 点拡がり関数
casa-type-sumwt = 重みの合計
casa-type-weight = 重み
label-casa-beam = 復元ビーム
casa-beam = { $major }″ × { $minor }″, 位置角 { $pa }°
label-casa-plane-beams = 平面ごとのビーム
casa-plane-beams = { $count } 個のビーム、長軸 { $min }″～{ $max }″
label-casa-rest-frequency = 静止周波数
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = スペクトル基準系
label-casa-velocity = 速度の定義
casa-velocity-radio = 電波
casa-velocity-optical = 光学
casa-velocity-relativistic = 相対論的
label-casa-mask = 既定のマスク
label-casa-storage = ストレージ
casa-storage-big = { $manager }、ビッグエンディアン
casa-storage-little = { $manager }、リトルエンディアン
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = メッシュ
label-ugrid-convention = 規約
label-ugrid-nodes = 節点
label-ugrid-faces = 面
label-ugrid-edges = 辺
label-ugrid-shapes = 面の形状
label-ugrid-coordinates = 座標
label-ugrid-values = 値の位置
label-ugrid-split = 描画用に分割
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
   *[other] { $count } 個の三角形
}
ugrid-quads = { $count ->
   *[other] { $count } 個の四角形
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
   *[other] { $count } 個の多角形 (最大 { $max } 節点)
}
ugrid-lonlat = 経度と緯度
ugrid-projected = 投影 x・y
ugrid-projected-unit = 投影 x・y ({ $unit })
ugrid-on-faces = 面 (塗りつぶし)
ugrid-on-nodes = 節点 (面上で補間)
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = なし: 画像は面ごとの節点数を表示
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
   *[other] 180 度経線または極にかかる { $count } 個の面
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = ファイル
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file } ({ $format })
label-annotation-shapes = 図形
label-annotation-groups = グループ
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = 省略
annotation-polygon = 多角形
annotation-polyline = 線
annotation-rectangle = 矩形
annotation-ellipse = 楕円
annotation-point = 点
annotation-points = 点群
annotation-ruler = ルーラー
annotation-arrow = 矢印
# The list of annotations is cut short: "and 1,200 more".
annotation-more = ほか { $count } 件
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = { $file } の注釈
label-drawing-version = バージョン
label-drawing-units = 単位
label-drawing-extents = 範囲
label-drawing-sheet = シート
label-drawing-layouts = レイアウト
label-drawing-layers = 画層
label-drawing-entities = 図形
label-drawing-codepage = コードページ
label-drawing-missing-fonts = 置換されたフォント
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file }（スタイル { $styles }）、{ $substitute } で描画
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file }（スタイル { $styles }）、代替フォントで描画
# Survey data (LandXML).
label-survey-crs = 座標参照系
survey-crs-none = 記載なし
label-survey-project = プロジェクト名
label-survey-application = 書き込みソフト
label-survey-surface = 地形サーフェス
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }：点 { $points }、三角形 { $triangles }；投影面積 { $area } { $unit }²、表面積 { $area3d } { $unit }²；最低点より上の体積 { $volume } { $unit }³；標高 { $zmin }–{ $zmax } { $unit }；等高線間隔 { $interval } { $unit }
label-survey-alignment = 線形
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }：{ $length } { $unit }、測点 { $start }～{ $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = SHX フォント
label-drawing-warnings = 問題
label-plot-paper = 用紙
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } / { $pages }
label-plot-page = ページ
label-plot-pens = 使用ペン
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = 印刷ジョブ
# What a plot draws: strokes, fills and labels.
label-plot-items = 描画要素
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = モデル
# "24, 3 off or frozen"
drawing-layers-off = { $num }(うち { $off } がオフまたはフリーズ)
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] インチ
    [2] フィート
    [3] マイル
    [4] ミリメートル
    [5] センチメートル
    [6] メートル
    [7] キロメートル
    [8] マイクロインチ
    [9] ミル
    [10] ヤード
    [11] オングストローム
    [12] ナノメートル
    [13] ミクロン
    [14] デシメートル
    [15] デカメートル
    [16] ヘクトメートル
    [17] ギガメートル
    [18] 天文単位
    [19] 光年
    [20] パーセク
    [21] US 測量フィート
   *[other] 単位なし
}
# All-fields group title: "DXF entities".
fields-drawing-entities = { $format } 図形

## IC レイアウト (GDSII)

# machine-drafted, needs native review
label-layout-library = ライブラリ
label-layout-units = 単位
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = データベース単位 { $db }、ユーザー単位 { $user }
# The top cell shown (each top cell is a page).
label-layout-cell = セル
label-layout-cells = セル数
# Cells no other cell places.
label-layout-top-cells = トップセル
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } ほか { $more } 個
# Layer/datatype pairs.
label-layout-layers = レイヤー
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = 図形
label-layout-texts = ラベル
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = 参照
layout-references = { $refs } ({ $placements } { $placements ->
   *[other] 配置
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = 平坦化後の図形
label-layout-bounds = 範囲
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } ～ { $x1 }, { $y1 } µm

## プリント基板と回路図 (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = 基板
group-schematic = 回路図
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = 表示
label-pcb-project = プロジェクト
label-pcb-revision = リビジョン
label-pcb-date = 日付
# The CAD program that wrote the file.
label-pcb-generator = 作成ソフト
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = 基板サイズ
label-pcb-thickness = 板厚
label-pcb-copper-layers = 銅箔層
label-pcb-sheets = シート
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = レイヤー
pcb-layer-role = { $name } ({ $role })
label-pcb-components = フットプリント
label-pcb-symbols = シンボル
label-pcb-nets = ネット
label-pcb-tracks = 配線
label-pcb-wires = ワイヤー
label-pcb-vias = ビア
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = 穴
pcb-holes = { $holes } (径 { $sizes } { $sizes ->
   *[other] 種類
})
label-pcb-apertures = アパーチャ
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = 内層銅箔 { $n }
pcb-role-top-copper = 表面銅箔
pcb-role-bottom-copper = 裏面銅箔
pcb-role-top-solder-mask = 表面ソルダーマスク
pcb-role-bottom-solder-mask = 裏面ソルダーマスク
pcb-role-top-silkscreen = 表面シルク
pcb-role-bottom-silkscreen = 裏面シルク
pcb-role-top-paste = 表面ペースト
pcb-role-bottom-paste = 裏面ペースト
pcb-role-top-solder-paste = 表面はんだペースト
pcb-role-bottom-solder-paste = 裏面はんだペースト
pcb-role-top-adhesive = 表面接着剤
pcb-role-bottom-adhesive = 裏面接着剤
pcb-role-top-courtyard = 表面コートヤード
pcb-role-bottom-courtyard = 裏面コートヤード
pcb-role-top-fabrication = 表面製造
pcb-role-bottom-fabrication = 裏面製造
pcb-role-board-outline = 基板外形
pcb-role-plated-drill = スルーホール (めっきあり)
pcb-role-non-plated-drill = 穴 (めっきなし)
pcb-role-drill = ドリル
pcb-role-drill-holes = ドリル穴
pcb-role-mechanical = メカニカル
pcb-role-margin = マージン
pcb-role-drawings = 図面
pcb-role-comments = コメント
pcb-role-engineering-change-1 = 設計変更 1
pcb-role-engineering-change-2 = 設計変更 2
pcb-role-vias = ビア
pcb-role-other = その他
pcb-role-unknown = 不明
label-pcb-macros = アパーチャマクロ

## スペクトル (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = スペクトル
label-spectrum-format = 形式
label-spectrum-technique = 測定法
label-spectrum-title = タイトル
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = ページ
spectrum-page-of = { $page }/{ $pages }: { $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = X 軸
label-spectrum-y = Y 軸
label-spectrum-nucleus = 核種
label-spectrum-frequency = 分光計の周波数
label-spectrum-solvent = 溶媒
label-spectrum-pulse-program = パルスプログラム
label-spectrum-spectra = スペクトル数
label-spectrum-points = 点数
label-spectrum-instrument = 装置
# Who or what lab measured it.
label-spectrum-origin = 測定元
label-spectrum-date = 測定日
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = 処理
spectrum-technique-ir = 赤外
spectrum-technique-raman = ラマン
spectrum-technique-uv-vis = 紫外可視
spectrum-technique-fluorescence = 蛍光
spectrum-technique-nmr = NMR
spectrum-technique-ms = マススペクトル
spectrum-technique-chromatogram = クロマトグラム
spectrum-technique-other = スペクトル

## 3D モデル (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = 3D モデル
label-triangles = 三角形
label-vertices = 頂点
label-materials = マテリアル
label-textures = テクスチャ
label-up-axis = 上方向の軸
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count } (欠落 { $missing })

## 分子構造 (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = 構造
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = エントリー
label-structure-classification = 分類
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = 手法
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = 分解能
structure-resolution = { $value } Å
label-structure-deposited = 登録
label-structure-formula = 化学式
label-structure-chains = 鎖
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = 残基
label-structure-atoms = 原子
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count } (非対称単位中 { $asymmetric })
label-structure-bonds = 結合
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = モデル
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = レコード
structure-first-of = { $count } (最初のものを表示)
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = リガンド
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } ほか { $more } 件
label-structure-cell = 単位格子
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = 空間群
label-structure-secondary = 二次構造
structure-ss-file = ファイルから
structure-ss-computed = 主鎖から計算

## HDF4 と HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = ライブラリ
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = データセット
label-hdf4-images = ラスター画像
label-hdf4-vgroups = グループ
label-hdf4-vdatas = テーブル
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = バージョン
# The grids, swaths and point structures the file defines.
label-eos-structures = 構造
eos-grids = { $count ->
   *[other] { $count }個のグリッド（{ $names }）
}
eos-swaths = { $count ->
   *[other] { $count }個のスワス（{ $names }）
}
eos-points = { $count ->
   *[other] { $count }個のポイント構造
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = 構造
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = 投影法
label-eos-grid-size = グリッドサイズ
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = 四隅
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = プロダクト
# When the data was acquired.
label-eos-time = 時刻
eos-time = { $begin }～{ $end }

## 惑星画像（VICAR、PDS3、PDS4：NASA・ESA のミッションデータ）

# machine-drafted, needs native review
group-planetary = 惑星画像
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = 対象天体
label-planetary-mission = ミッション
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = 探査機
label-planetary-start = 開始時刻
label-planetary-stop = 終了時刻
# The product's identifier in the archive.
label-planetary-product = プロダクト ID
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = 地図投影法
label-planetary-resolution = 分解能
planetary-metres-per-pixel = 1 ピクセルあたり { $metres } m
planetary-pixels-per-degree = 1 度あたり { $pixels } ピクセル
# Latitude and longitude of the map projection's centre.
label-planetary-centre = 投影中心
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = 範囲
planetary-coverage = 緯度 { $south }°～{ $north }°、経度 { $west }°～{ $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = 天体半径
planetary-km = { $km } km

## OMERO（OMERO サーバーまたは Image Data Resource から開いた画像）

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = サーバー
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server }（{ $user } として）
omero-server-public = { $server }（公開）
label-omero-image = 画像
# $name the image's name; $id its number on the server.
omero-image = { $name }（ID { $id }）
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = プロジェクト
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = スクリーン
label-omero-owner = 所有者
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner }（{ $group }）
label-omero-acquired = 取得日時
label-omero-dimensions = サイズ
omero-size = { $width } × { $height }
omero-z = { $count ->
   *[other] { $count } Z 平面
}
omero-c = { $count ->
   *[other] { $count } チャンネル
}
omero-t = { $count ->
   *[other] { $count } 時点
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = ピクセルサイズ
label-omero-channels = チャンネル
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel }（非表示）
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = 取得方法
omero-fetched-tiles = { $encoding } タイル { $width } × { $height }、{ $levels ->
   *[other] { $levels } レベル
}
omero-fetched-whole = 平面全体を { $encoding } で

## 反射変換画像 (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = 反射変換
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = 基底
label-rti-order = 次数
# Coefficients per colour channel.
label-rti-terms = 係数
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = 色の配置
# Coefficient images stored in the file.
label-rti-planes = プレーン
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = スケール
label-rti-bias = バイアス

## 点群

# machine-drafted, needs native review
group-points = 点群
label-points = 点数
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $total } 中 { $loaded }
# The LAS point data record format number (0–10).
label-point-format = ポイントフォーマット
# The coordinate reference system.
label-crs = 座標参照系
# What each point carries besides its position.
label-attributes = 属性
points-channel-rgb = RGB
points-channel-intensity = 強度
points-channel-classification = 分類
points-channel-returns = リターン
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = クラス
# E57 scans, with point counts.
label-scans = スキャン
point-class-0 = 未分類 (処理なし)
point-class-1 = 未分類
point-class-2 = 地表面
point-class-3 = 低植生
point-class-4 = 中植生
point-class-5 = 高植生
point-class-6 = 建物
point-class-7 = 低点 (ノイズ)
point-class-8 = モデルキーポイント
point-class-9 = 水域
point-class-10 = 鉄道
point-class-11 = 路面
point-class-12 = オーバーラップ
point-class-13 = 架空地線
point-class-14 = 電線
point-class-15 = 送電鉄塔
point-class-16 = 電線接続具
point-class-17 = 橋床
point-class-18 = 高ノイズ
point-class-19 = 頭上構造物
point-class-20 = 無視された地表面
point-class-21 = 雪
point-class-22 = 時間的除外
# A class without a standard name, by number.
point-class-other = クラス { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = ガウシアンスプラット
label-splats = スプラット
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } / { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = 視点依存の色
splats-harmonics-none = なし
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }、アンチエイリアスありで学習

## 警告

# machine-drafted, needs native review
warn-not-diagnostic = 診断には使用しないでください。Specular は汎用の画像ビューアーであり、医療機器ではありません。表示は診断用に校正されていません。

warn-gps = このファイルには撮影場所が記録されています。公開したくない場合は、共有する前に位置情報を削除してください。
warn-null-island = GPS 位置がちょうど 0°、0° です。カメラが測位できていなかった可能性が高く、位置情報はおそらく正しくありません。
warn-date-future = 撮影日時が未来の日付になっています。カメラの時計の設定が誤っていた可能性があります。
warn-icc-damaged = 埋め込まれたカラープロファイルが破損しているため、読み込めませんでした。
warn-metadata-failed = Specular のエラーのため、メタデータを読み取れませんでした。画像自体には問題ありません。
warn-orientation-conflict = 向きの情報が EXIF と XMP で一致しません（EXIF：{ $exif }、XMP：{ $xmp }）。EXIF の値を使用します。
warn-exif-damaged = EXIF データの一部を読み込めませんでした（{ $error }）。残りの情報を表示しています。
warn-xmp-damaged = XMP メタデータの形式が正しくありません（{ $error }）。一部のみ読み込みました。

## バージョン情報

geonames-attribution = 地名：GeoNames（www.geonames.org）、Creative Commons 表示 4.0（CC BY 4.0、https://creativecommons.org/licenses/by/4.0/）の下でライセンスされています。オフライン検索には Grant Miner による reverse_geocoder クレート（MIT または Apache-2.0）を使用しています。データは都市一覧に変換されており、現状のまま無保証で提供されます。

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] フル解像度の画像
    [1] 縮小解像度の画像
    [2] 複数ページ画像の 1 ページ
    [4] 透明マスク
   *[other] その他
}
predictor = { $value ->
    [1] なし
    [2] 水平差分
    [3] 浮動小数点差分
   *[other] その他
}
extra-samples = { $value ->
    [0] 指定なし
    [1] 乗算済みアルファ
    [2] ストレートアルファ
   *[other] その他
}
sample-format = { $value ->
    [1] 符号なし整数
    [2] 符号付き整数
    [3] 浮動小数点
    [4] 未定義
   *[other] その他
}

## 地理空間データ (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = 地理空間
label-geo-crs = 座標参照系
label-geo-datum = 測地系
label-geo-projection = 投影法
label-geo-unit = 単位
label-geo-bounds = 範囲
label-geo-lonlat-bounds = 経度と緯度
label-geo-pixel-size = ピクセルサイズ
label-geo-overviews = オーバービュー
label-geo-nodata = NoData 値
label-geo-raster-type = ラスタータイプ
label-geo-vertical = 鉛直基準
label-geo-citation = 引用
label-geo-features = 地物
label-geo-vertices = 頂点
label-geo-layer = レイヤー
label-geo-layers = レイヤー
label-geo-tiles = タイル
label-geo-encoding = 文字エンコーディング
label-geo-warnings = 問題
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = 付随ファイル
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method } (座標には非対応)
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = WGS 84 への変換
geo-to-wgs84-same = 不要
geo-to-wgs84-helmert = 測地系変換 (Helmert)
geo-to-wgs84-by-area = 測地系変換 (Helmert、地域で選択)
geo-to-wgs84-unknown = 不明: 座標はファイル独自の測地系です
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = 記載なし
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } – { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = 経度 { $west }°–{ $east }°、緯度 { $south }°–{ $north }°
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
   *[other] オーバービュー { $n } 個: { $sizes }
}
geo-pixel-is-point = ピクセルは点
geo-layer = { $features ->
   *[other] 地物 { $features } 個
}、{ $fields ->
   *[other] フィールド { $fields } 個
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }、ズーム { $zoom }、{ $tiles ->
   *[other] タイル { $tiles } 枚
}
fields-geo-geokeys = GeoTIFF キー
fields-geo-gdal = GDAL メタデータ
fields-geo-wkt = 座標参照系 (WKT)
fields-geo-projjson = 座標参照系 (PROJJSON)
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = タイルセット
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = クレジット
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = ヘッダー
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = GIFTI バージョン
label-gifti-role = 内容
gifti-role-surface = 表面
gifti-role-labels = 頂点ごとのラベル
gifti-role-data = 頂点ごとの値
label-gifti-arrays = データ配列
label-gifti-encoding = エンコード
label-gifti-vertices = 頂点
label-gifti-triangles = 三角形
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = 構造
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = 幾何タイプ
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = 座標
gifti-transform = { $from } ({ $to } への変換は不要)
gifti-transform-applied = { $from } → { $to } (適用済み)
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = 表示先
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = 配列
label-gifti-intent = Intent
label-gifti-data-type = データ型
# Entries of the label table (names and colours of regions).
label-gifti-labels = ラベルテーブル
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = CIFTI バージョン
label-cifti-intent = Intent
label-cifti-matrix = 行列
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = マップ
cifti-maps-scalars = スカラーマップ { $count } 個
cifti-maps-labels = ラベルマップ { $count } 個
cifti-maps-series = 時点 { $count } 個
cifti-maps-parcels = パーセル { $count } 個
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = 結合性
cifti-dense = シード { $seeds } 個、シードごとに { $size } 読み込み
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = 系列
cifti-series = { $start } から { $step } ごと ({ $unit })
label-cifti-models = 脳モデル
label-cifti-parcels = パーセル
label-cifti-structures = 構造
label-cifti-voxels = 皮質下ボクセル
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = 表面
cifti-surface-found = { $structure }: 頂点 { $vertices } 個、{ $file }
cifti-surface-missing = { $structure }: 頂点数 { $vertices } の表面が必要 (ファイルの隣にありません)
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = 表示方法
cifti-shown-surfaces = 着色された表面
cifti-shown-voxels = 皮質下ボクセルのボリューム
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = 選択したシードの結合性
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = 除外したマップ
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = 形式
label-tract-streamlines = ストリームライン
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count } ({ $shown } 本を表示)
label-tract-points = 点
label-tract-mean-length = 平均長
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = 点ごとのスカラー
label-tract-properties = ストリームラインごとのプロパティ
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = グリッド
label-tract-voxel-size = ボクセルサイズ
label-tract-voxel-order = ボクセル順序
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = 座標
tract-space-vox-to-ras = ボクセル→RAS 行列から
tract-space-voxel-order = ボクセル順序から (ボクセル→RAS 行列なし)
tract-space-scanner = スキャナー空間
# MRtrix: the tracking step size.
label-tract-step = ステップサイズ

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = ファイルヘッダー
label-header-created = 作成日時
# The program or person that wrote the file.
label-header-creator = 作成者
label-header-copyright = 著作権
label-header-project = プロジェクト
label-header-comment = コメント
# The scanner, camera or other device the image came from.
label-header-device = デバイス
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = タイムコード
# A film frame's edge code (KeyKode).
label-header-edge-code = エッジコード
label-header-frame-rate = フレームレート
# Where the frame sits in its sequence.
label-header-frame = フレーム
# Film stock and slate.
label-header-film = フィルム
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = 伝達関数
# The colour primaries the file states.
label-header-colorimetric = 測色
# The X window a screen dump was taken of.
label-header-window = ウィンドウ
label-header-resolution = 分解能
label-header-compression = 圧縮
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = プロファイル
# machine-drafted, needs native review
label-header-level = レベル
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = ウェーブレットレベル
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = 色変換
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = アートボード
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = 描画されない
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = 近似で描画

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = フォント
label-font-family = ファミリー
label-font-style = スタイル
# The face's place in a font collection: "2 of 4".
label-font-face = フェイス
font-face-of = { $index } / { $count }
label-font-version = バージョン
label-font-designer = デザイナー
# The font's maker (the name table's manufacturer).
label-font-manufacturer = 制作会社
label-font-glyphs = グリフ
label-font-outlines = アウトライン
font-outlines-truetype = TrueType（2 次）
font-outlines-cff = PostScript（CFF、3 次）
font-outlines-cff2 = PostScript（CFF2、3 次）
font-outlines-bitmap = ビットマップのみ
font-outlines-none = なし
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = 埋め込み
font-embedding-installable = 埋め込みとインストールが可能。
font-embedding-restricted = 埋め込み不可。
font-embedding-preview-print = 表示と印刷のためだけに埋め込み可能。
font-embedding-editable = 編集する文書に埋め込み可能。
font-embedding-no-subsetting = サブセット化せず全体のみ。
font-embedding-bitmap-only = ビットマップとしてのみ。
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = バリエーション軸
font-axis = { $name } { $min }–{ $max }（{ $default }）
label-font-instances = 名前付きスタイル
label-font-colour = カラーグリフ
font-colour-colr0 = COLR レイヤー
font-colour-colr1 = COLR v1 グラデーション
font-colour-sbix = ビットマップ（sbix）
font-colour-cbdt = ビットマップ（CBDT）
font-colour-svg = SVG
label-font-weight = ウェイト
label-font-units-per-em = em あたりのユニット数
label-font-tables = テーブル
label-font-licence = ライセンス
label-font-licence-url = ライセンス URL
label-font-copyright = 著作権
label-font-trademark = 商標

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = ドキュメント
label-document-kind = 種類
document-kind-word = Word 文書
document-kind-excel = Excel ブック
document-kind-powerpoint = PowerPoint プレゼンテーション
document-kind-pages = Pages 書類
document-kind-numbers = Numbers スプレッドシート
document-kind-keynote = Keynote プレゼンテーション
document-kind-odf-text = OpenDocument テキスト
document-kind-odf-spreadsheet = OpenDocument スプレッドシート
document-kind-odf-presentation = OpenDocument プレゼンテーション
document-kind-odf-graphics = OpenDocument 図形描画
document-kind-odf-other = OpenDocument ファイル
# machine-drafted, needs native review
document-kind-playlist = プレイリスト
# machine-drafted, needs native review
label-playlist-items = 項目
# Appended to the kind: "Word document, template, with macros".
document-template = テンプレート
document-macros = マクロあり
document-legacy = Office 97–2003 形式
label-document-preview = プレビュー
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = { $format } 画像
document-preview-none = このファイルにはありません
label-document-subject = 件名
label-document-author = 作成者
label-document-description = 説明
label-document-category = 分類
label-document-last-saved-by = 最終保存者
label-document-created = 作成日時
label-document-modified = 更新日時
label-document-printed = 最終印刷日時
label-document-pages = ページ数
label-document-slides = スライド数
label-document-hidden-slides = 非表示スライド数
label-document-notes = ノート ページ数
label-document-sheets = シート数
label-document-words = 単語数
label-document-characters = 文字数
label-document-paragraphs = 段落数
label-document-lines = 行数
label-document-editing-time = 編集時間
document-minutes = { $minutes } 分
label-document-revision = 改訂番号
label-document-application = 保存したアプリ
label-document-company = 会社
label-document-template = テンプレート

## Books (EPUB)

# machine-drafted, needs native review
group-book = 本
label-book-authors = 著者
label-book-publisher = 出版社
label-book-date = 発行日
label-book-language = 言語
label-book-pages = ページ数
# Entries in the book's table of contents.
label-book-contents = 目次の項目数
label-book-layout = レイアウト
book-layout-reflowable = リフロー
book-layout-fixed = 固定レイアウト
book-right-to-left = 右から左
label-book-version = 形式
book-version = EPUB { $version }
label-book-fonts = 埋め込みフォント
book-fonts-obfuscated = { $count }（難読化 { $obfuscated }）
label-book-subjects = 主題
label-book-identifier = 識別子
label-book-rights = 権利

unit-symbol-mm = mm
unit-symbol-cm = cm
unit-symbol-m = m
unit-symbol-km = km
unit-symbol-mm2 = mm²
unit-symbol-cm2 = cm²
unit-symbol-m2 = m²
unit-symbol-km2 = km²
unit-symbol-ha = ha
unit-symbol-ft = ft
unit-symbol-ft2 = ft²
unit-symbol-mi = mi
unit-symbol-mi2 = mi²
unit-symbol-ac = ac

# machine-drafted, needs native review
volume-orient-parrec = Philips の角度とオフセンター
volume-orient-ecat = ECAT のボクセルサイズとオフセット（信頼性なし）
label-volume-technique = 撮像法
label-volume-version = 形式のバージョン
label-volume-isotope = 核種
label-volume-tracer = トレーサー
label-volume-units = 単位

# Text, tables and subtitles.
label-header-encoding = 文字コード
label-header-lines = 行数
label-header-line-ends = 改行コード
label-header-words = 単語数
label-header-pages = ページ数
label-header-pictures = 画像
label-header-pictures-not-shown = 表示されない画像
label-header-tables = 表
label-header-rows = 表の行数
label-header-columns = 列数
label-header-delimiter = 区切り文字
label-header-column-names = 列名
label-header-cues = 字幕
label-header-cue-span = 字幕の範囲
