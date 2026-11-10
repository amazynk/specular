# Specular — File kinds: which format a shared extension is, Open as…, remembered choices. 日本語.

## Which kind of file? Files whose extension several formats share.

# machine-drafted, needs native review
file-kind-title = { $file } はどの種類のファイルですか？
# machine-drafted, needs native review
file-kind-why-content = 内容が複数のフォーマットに当てはまります。
# machine-drafted, needs native review
file-kind-why-name = 内容からは判断できません。これらのフォーマットが .{ $ext } ファイルを使います。
# machine-drafted, needs native review
file-kind-why-none = 内容を認識できるフォーマットがありません。
# machine-drafted, needs native review
file-kind-certain = 確実
# machine-drafted, needs native review
file-kind-likely = 可能性が高い
# machine-drafted, needs native review
file-kind-possible = 可能性あり
# machine-drafted, needs native review
file-kind-guess = 名前のみで判断
# machine-drafted, needs native review
file-kind-hint-same = ここにある他の .{ $ext } ファイルと同じ
# machine-drafted, needs native review
file-kind-hint-companions = いつもの付属ファイルが隣にあります
# machine-drafted, needs native review
file-kind-scope = 適用範囲
# machine-drafted, needs native review
file-kind-scope-file = このファイル
# machine-drafted, needs native review
file-kind-scope-folder = このフォルダーの .{ $ext } ファイル
# machine-drafted, needs native review
file-kind-scope-everywhere = すべての .{ $ext } ファイル
# machine-drafted, needs native review
file-kind-later = あとでファイルのメニューの「形式を指定して開く…」で変更できます。
# machine-drafted, needs native review
file-kind-open = 開く
# machine-drafted, needs native review
file-kind-raw = 生のサンプル
# machine-drafted, needs native review
file-kind-open-as-none = { $file } を読み込めるものは他にありません。
# machine-drafted, needs native review
error-reason-ambiguous = 複数のフォーマットのどれかかもしれません。1 つ選んで開いてください。

## Open as… (the palette and the gallery's file menu)

# machine-drafted, needs native review
palette-open-as = 形式を指定して開く…
# machine-drafted, needs native review
palette-open-as-help = ファイルをどのフォーマットとして読み込むかを選びます。

## Settings › Optional decoders › File kinds

# machine-drafted, needs native review
settings-decoders-file-kinds = すべてのファイルの種類を消去…
# machine-drafted, needs native review
settings-decoders-file-kinds-help = 複数のフォーマットが共有する拡張子に対して選んだフォーマットです。次回また確認します。
# machine-drafted, needs native review
file-kind-rules-intro = 各ルールは、ある拡張子のファイルを 1 つのフォーマットとして開きます。
# machine-drafted, needs native review
file-kind-rules-none = ルールはまだありません。確認されたときに「このフォルダーの .ext ファイル」か「すべての .ext ファイル」を選んでください。
# machine-drafted, needs native review
file-kind-rule = .{ $ext } ファイル：{ $format }
# machine-drafted, needs native review
file-kind-rule-folder = { $folder } 内
# machine-drafted, needs native review
file-kind-rule-everywhere = すべての場所
# machine-drafted, needs native review
file-kind-forget = 消去…
# machine-drafted, needs native review
file-kind-forget-ask = 「.{ $ext } ファイル：{ $format }」を消去しますか？
# machine-drafted, needs native review
file-kind-forget-detail = 次回 { -app-name } がまた確認します。ファイルが変更・削除されることはありません。
# machine-drafted, needs native review
file-kind-forget-ok = 消去
# machine-drafted, needs native review
file-kind-forgotten = .{ $ext } ファイル：{ $format } を消去しました。
# machine-drafted, needs native review
file-kind-forget-all-ask = { $count ->
   *[other] { $count } 件のファイルの種類のルールを消去しますか？
}
# machine-drafted, needs native review
file-kind-forgotten-all = { $count ->
   *[other] { $count } 件のファイルの種類のルールを消去しました。
}
# machine-drafted, needs native review
file-kind-keywords = 拡張子, フォーマット, 形式を指定して開く, 種類, ファイルの種類

## One line on what each format is, under its name in the prompt.

# machine-drafted, needs native review
file-kind-about-other = .{ $ext } ファイルを使う別のフォーマット。
# machine-drafted, needs native review
file-kind-about-raw-volume = ヘッダーのないサンプル。幅、高さ、スライス数、サンプル型を指定します。
# machine-drafted, needs native review
file-kind-about-raw = カメラの RAW 写真。
# machine-drafted, needs native review
file-kind-about-flir-fff = 温度情報付きの FLIR 熱画像。
# machine-drafted, needs native review
file-kind-about-fits = 天文画像 (FITS)。
# machine-drafted, needs native review
file-kind-about-fit = Garmin のアクティビティ記録：GPS ポイントとセンサーデータ。
# machine-drafted, needs native review
file-kind-about-mif = MRtrix 医用ボリューム (MRI)。
# machine-drafted, needs native review
file-kind-about-map-info-mif = .mid データ付きの MapInfo ベクター地図。
# machine-drafted, needs native review
file-kind-about-net-cdf = 科学用グリッドデータ (気候、海洋)。
# machine-drafted, needs native review
file-kind-about-hdf5 = HDF5 科学データ。
# machine-drafted, needs native review
file-kind-about-playlist = オーディオプレイリスト (PLS)。
# machine-drafted, needs native review
file-kind-about-gerber = 基板レイヤー (Gerber)、はんだペーストなど。
# machine-drafted, needs native review
file-kind-about-ser = 惑星カメラの天体動画 (SER)。
# machine-drafted, needs native review
file-kind-about-tia = 電子顕微鏡シリーズ (FEI TIA)。
file-kind-about-pcd = 3D 点群 (PCD)。
file-kind-about-photo-cd = Kodak Photo CD のスキャン。
file-kind-about-zx-screen = ZX Spectrum の画面ダンプ。
file-kind-about-cpc-screen = Amstrad CPC の画面ダンプ。
file-kind-about-msx-screen = MSX の画面ダンプ。
file-kind-about-scc = Scenarist のクローズドキャプション。
# machine-drafted, needs native review
file-kind-about-pict = クラシック Mac の画像 (PICT)。
# machine-drafted, needs native review
file-kind-about-hdr = Radiance HDR 画像。
# machine-drafted, needs native review
file-kind-about-analyze = .hdr 付きの Analyze 7.5 医用ボリューム。
# machine-drafted, needs native review
file-kind-about-envi = .hdr 付きのリモートセンシングラスター (ENVI)。
# machine-drafted, needs native review
file-kind-about-pds3 = ラベル付きの惑星探査画像 (PDS3)。
# machine-drafted, needs native review
file-kind-about-mpeg-ps = ビデオカメラの動画 (MPEG プログラムストリーム、MOD/TOD)。
# machine-drafted, needs native review
file-kind-about-tracker = トラッカー音楽モジュール (MOD、S3M、XM、IT、669、MTM、STM、OKT、MED、ULT、FAR)。
# machine-drafted, needs native review
file-kind-about-softimage-pic = Softimage の 3D レンダリング画像 (PIC)。
# machine-drafted, needs native review
file-kind-about-dr-halo = .pal パレット付きの Dr. Halo 画像。
# machine-drafted, needs native review
file-kind-about-wal = ゲームのパレットで色付けされる Quake 2 テクスチャ。
# machine-drafted, needs native review
file-kind-about-quick-time = QuickTime ムービー。
file-kind-about-pro-res-raw = Apple ProRes RAW 動画：カメラのセンサーのフレーム。
file-kind-about-cinema-raw-light = Canon Cinema RAW Light 動画：カメラのセンサーのフレーム。
# machine-drafted, needs native review
file-kind-about-gimp-brush = GIMP ブラシ。
# machine-drafted, needs native review
file-kind-about-swatch-exchange = Adobe Swatch Exchange カラーパレット。
# machine-drafted, needs native review
file-kind-about-aseprite = Aseprite のピクセルアート スプライト。
