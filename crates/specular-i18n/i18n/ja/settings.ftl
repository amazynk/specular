# machine-drafted, needs native review
# Specular — 設定ウインドウ。日本語。

## ウインドウ

settings-title = 設定
settings-search-placeholder = 設定を検索
settings-search-no-results = { $query } に一致する設定はありません
settings-reset = リセット
settings-reset-section = セクションをリセット
settings-modified = 変更済み
settings-default-value = デフォルト：{ $value }
settings-restart-required = 再起動後に有効になります
settings-restart-now = 今すぐ再起動
settings-undo = 取り消す
settings-redo = やり直す
settings-value-on = オン
settings-value-off = オフ
settings-value-auto = 自動
settings-value-system = システムに従う
settings-value-system-language = システムのデフォルト
settings-value-follow-language = インターフェイスの言語に従う
settings-value-default-location = デフォルトの場所
settings-choose = 選択…
settings-clear = クリア
settings-file-reloaded = settings.toml が { -app-name } の外部で変更されたため、再読み込みしました。
settings-warning-unknown-key = settings.toml に不明な設定 { $key } があります。ファイルには残されますが、無視されます。
settings-warning-invalid-value = { $key } の値が無効です：{ $reason } 代わりにデフォルト値が使用されます。
settings-warning-newer-version = settings.toml は新しいバージョンの { -app-name } で書き込まれています。このバージョンが認識できない設定はそのまま保持されます。
settings-warning-syntax = settings.toml を読み込めません（{ $line } 行目）：{ $reason } 修正されるまでデフォルト値が使用されます。

## 検証エラー（`specular --set` でも出力されます）

settings-error-unknown-key = { $key } という設定はありません。
settings-error-unknown-key-suggestion = { $key } という設定はありません。{ $suggestion } のことですか？
settings-error-not-bool = on または off（true または false）を指定してください。
settings-error-not-integer = 整数を指定してください。
settings-error-not-number = 数値を指定してください。
settings-error-out-of-range = { $min } から { $max } の範囲で指定してください。
settings-error-out-of-range-auto = 0（自動）または { $min } から { $max } の範囲で指定してください。
settings-error-unknown-option = 次のいずれかを指定してください：{ $options }。
settings-error-bad-colour = #1E1E1E や #1E1E1E80 のようなカラーを指定してください。
settings-error-bad-language = de、pt-BR、zh-Hans のような言語タグ、または “system” を指定してください。
settings-error-bad-path = ファイルまたはフォルダのパスを指定してください。
settings-error-not-settable = この項目はコマンドのため、値はありません。
settings-error-syntax = key=value の形式で指定してください（例：viewing.wheel_behaviour=navigate）。
# machine-drafted, needs native review
settings-error-bad-text = タブ、改行、その他の制御文字は使えません。
settings-error-invalid = { $reason }
settings-error-tile-url = {"{"}z{"}"}、{"{"}x{"}"}、{"{"}y{"}"} を含む http または https のアドレスが必要です。

## 単位

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } ms
unit-seconds = { $value } 秒
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
       *[other] { $value } 枚
    }
unit-threads =
    { $value ->
       *[other] { $value } スレッド
    }
unit-bytes =
    { $value ->
       *[other] { $value } バイト
    }
unit-kilobytes = { $value } KB
unit-megabytes = { $value } MB
unit-gigabytes = { $value } GB
unit-terabytes = { $value } TB
unit-kibibytes = { $value } KiB
unit-gibibytes = { $value } GiB
unit-tebibytes = { $value } TiB
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

## セクション

settings-section-general = 一般
settings-section-viewing = 表示
settings-section-browsing = ブラウズ
settings-section-colour = カラー
settings-section-metadata = メタデータとプライバシー
# machine-drafted, needs native review
settings-section-privacy = プライバシー
settings-section-performance = パフォーマンス
settings-section-accessibility = アクセシビリティ
settings-section-keys = キー
settings-section-advanced = 詳細

## 一般

settings-general-language = インターフェイスの言語
settings-general-language-help = 変更はすぐに反映されます。「システムのデフォルト」を選ぶと、オペレーティングシステムの言語に従います。
settings-general-region-formats = 地域のフォーマット
settings-general-region-formats-help = 日付、時刻、数値、サイズの表記方法です。デフォルトではインターフェイスの言語に従います。
settings-general-theme = テーマ
settings-general-theme-help = ウインドウやコントロールのライトまたはダークの外観です。画像は変更されません。
settings-general-theme-system = システムに従う
settings-general-theme-light = ライト
settings-general-theme-dark = ダーク
# machine-drafted, needs native review
settings-general-on-start = ファイルを指定せずに { -app-name } を開いたとき
settings-general-on-start-help = { -app-name } を単独で開いたときに表示する内容です。ファイルマネージャーやコマンドラインから開いたファイルは常にビューアーで表示されます。
settings-general-on-start-start-screen = スタート画面
settings-general-on-start-gallery = 最後のフォルダーのギャラリー
settings-general-on-start-library = ライブラリ
settings-general-start-tips = スタート画面に「試してみる」を表示
settings-general-start-tips-help = 便利な機能とそのキーを示すカードの列です。コマンドパレットの「ようこそを表示」で戻せます。
settings-general-tips = ヒントを表示
settings-general-tips-help = 機能が役立ちそうなときに一度だけ表示する短いヒントです (フォルダーをギャラリーで表示するキーなど)。スライドショー中には表示しません。
settings-general-tips-shown = 表示済みのヒント
settings-general-tips-shown-help = すでに表示した一度だけのヒント。再表示しないために記録します。
# machine-drafted, needs native review
settings-general-your-name = 名前
settings-general-your-name-help = 追加したスタンプとマークアップに表示されます。保存先は作成したマークアップファイルのみです。
settings-general-single-instance = 既存のウインドウでファイルを開く
settings-general-single-instance-help = { -app-name } がすでに起動している場合、別の画像を開くと新しく起動せずにそのウインドウに表示します。
settings-general-file-associations = ファイルの関連付け…
settings-general-file-associations-help = { -app-name } を、対応する画像フォーマットのデフォルトのビューアにします。

## 表示

settings-viewing-initial-zoom = 初期の拡大率
settings-viewing-initial-zoom-help = 新しく開いた画像をどの大きさで表示するかを指定します。
settings-viewing-initial-zoom-fit = ウインドウに合わせる
settings-viewing-initial-zoom-fit-if-larger = 大きな画像は合わせ、小さな画像は { PERCENT(100) } で表示
settings-viewing-initial-zoom-actual-size = 実際のサイズ（{ PERCENT(100) }）
settings-viewing-initial-zoom-fill = ウインドウを埋める
settings-viewing-wheel-behaviour = マウスホイール
settings-viewing-wheel-behaviour-help = ホイールを回したときの動作です。Ctrl キーを押しながら回すと、もう一方の動作になります。
settings-viewing-wheel-behaviour-help-macos = ホイールを回したときの動作です。⌘ キーを押しながら回すと、もう一方の動作になります。
settings-viewing-wheel-behaviour-zoom = 拡大・縮小
settings-viewing-wheel-behaviour-navigate = 次／前の画像
settings-viewing-keep-zoom = 画像間でズームを保持
settings-viewing-keep-zoom-help = 拡大中は、次と前の画像が同じズームで開き、フレームの同じ部分を表示します。連写のピント確認に便利です。
settings-viewing-zoom-animation = 拡大・縮小をアニメーション
settings-viewing-zoom-animation-help = 拡大・縮小を瞬時に切り替えず、滑らかに行います。「視差効果を減らす」がオンのときは自動的にオフになります。
settings-viewing-zoom-animation-speed = 拡大・縮小アニメーションの速度
settings-viewing-zoom-animation-speed-help = アニメーションによる拡大・縮小が完了するまでの速さです。
settings-viewing-zoom-animation-speed-relaxed = ゆっくり
settings-viewing-zoom-animation-speed-normal = 標準
settings-viewing-zoom-animation-speed-fast = 速い
settings-viewing-background = 背景
settings-viewing-background-help = ウインドウ内で画像の背後に表示される色です。
settings-viewing-background-theme = テーマに合わせる
settings-viewing-background-black = 黒
settings-viewing-background-dark-grey = ダークグレイ
settings-viewing-background-light-grey = ライトグレイ
settings-viewing-background-white = 白
settings-viewing-background-custom = カスタムカラー
settings-viewing-background-color = カスタム背景色
settings-viewing-background-color-help = 「背景」が「カスタムカラー」に設定されているときに使用されます。
settings-viewing-fullscreen-background = フルスクリーンの背景
settings-viewing-fullscreen-background-help = フルスクリーンで画像の背後に表示される色です。
settings-viewing-transparency = 透明な部分
settings-viewing-transparency-help = 画像の透明な部分の描画方法です。
settings-viewing-transparency-checkerboard = 市松模様
settings-viewing-transparency-background = 背景色
settings-viewing-transparency-black = 黒
settings-viewing-transparency-white = 白
settings-viewing-pixel-grid-threshold = ピクセルグリッドを表示する拡大率
settings-viewing-pixel-grid-threshold-help = この拡大率以上に拡大したとき、ピクセル間にグリッドを描画します。
settings-viewing-smoothing-high-zoom = 拡大したピクセル
settings-viewing-smoothing-high-zoom-help = 画像を { PERCENT(100) } より大きく表示したときのピクセルの見え方です。
settings-viewing-smoothing-high-zoom-auto = 滑らか、{ PERCENT(400) } 以上でシャープ
settings-viewing-smoothing-high-zoom-smooth = 常に滑らか
settings-viewing-smoothing-high-zoom-pixelated = 常にシャープな正方形
settings-viewing-double-click = 画像をダブルクリックしたとき
settings-viewing-double-click-help = 何もない背景をダブルクリックすると、常にフルスクリーンが切り替わります。
settings-viewing-double-click-toggle-zoom = ウインドウに合わせる／{ PERCENT(100) } を切り替え
settings-viewing-double-click-fullscreen = フルスクリーンの切り替え
settings-viewing-double-click-nothing = 何もしない
settings-viewing-fullscreen-on-open = 画像をフルスクリーンで開く
settings-viewing-fullscreen-on-open-help = { -app-name } の外部から画像を開いたとき、フルスクリーンで開始します。
settings-viewing-fullscreen-mode = フルスクリーンの方式
settings-viewing-fullscreen-mode-help = ネイティブのフルスクリーンは専用の操作スペースを使用します。インスタントのフルスクリーンはアニメーションなしで現在の操作スペースを覆います。
settings-viewing-fullscreen-mode-native = ネイティブ（専用の操作スペース）
settings-viewing-fullscreen-mode-borderless = インスタント
settings-viewing-toolbar-autohide = ツールバーを自動的に隠す
settings-viewing-toolbar-autohide-help = ポインタが止まるとツールバーと端の矢印がフェードアウトし、動かすと再び表示されます。
settings-viewing-toolbar-autohide-delay = ツールバーを隠すまでの時間
settings-viewing-toolbar-autohide-delay-help = ツールバーがフェードアウトするまでにポインタを止めておく時間です。
settings-viewing-toolbar-items = ツールバーのボタン
settings-viewing-toolbar-items-help = ツールバーに表示するボタンとその順序です。
settings-viewing-toolbar-drag = ドラッグしてツールバーを並べ替える
settings-viewing-toolbar-drag-help = ツールバー上でボタンをドラッグすると移動できます。短いクリックは通常どおりボタンを押します。Esc でドラッグ中のボタンを元に戻します。
settings-viewing-toolbar-items-open = 開く
settings-viewing-toolbar-items-prev = 前の画像
settings-viewing-toolbar-items-next = 次の画像
settings-viewing-toolbar-items-zoom-out = 縮小
settings-viewing-toolbar-items-zoom-level = 拡大率
settings-viewing-toolbar-items-zoom-in = 拡大
settings-viewing-toolbar-items-fit = ウインドウに合わせる
settings-viewing-toolbar-items-rotate-left = 左に回転
settings-viewing-toolbar-items-rotate-right = 右に回転
settings-viewing-toolbar-items-info = 情報
settings-viewing-toolbar-items-copy = 画像をコピー
settings-viewing-toolbar-items-delete = ゴミ箱に移動
settings-viewing-toolbar-items-palette = コマンドパレット
settings-viewing-toolbar-items-settings = 設定
settings-viewing-toolbar-items-fullscreen = フルスクリーン
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = 可変スペース
settings-viewing-toolbar-items-separator = 区切り線
settings-toolbar-preview = ツールバーのプレビュー
settings-toolbar-hidden = 非表示のボタン
settings-toolbar-add-spacer = 可変スペースを追加
settings-toolbar-add-separator = 区切り線を追加
settings-toolbar-reset = 既定に戻す
settings-toolbar-remove = { $item }を削除
settings-toolbar-hint = 行をドラッグして移動するか、行を選択して Alt+↑ または Alt+↓ を押します。Tab で検索フィールドとリストを移動します。
settings-toolbar-move-up = 上へ移動
settings-toolbar-move-down = 下へ移動
settings-toolbar-move-start = 先頭へ移動
settings-toolbar-move-end = 末尾へ移動
settings-toolbar-menu-hide = 非表示にする
settings-toolbar-menu-show = 表示する
settings-toolbar-menu-remove = 削除
settings-toolbar-row-menu = { $item }のその他の操作
settings-toolbar-position = { $count } 個中 { $index } 番目
settings-toolbar-hidden-state = 非表示

## ブラウズ

settings-browsing-sort-order = 画像の並べ替え
settings-browsing-sort-order-help = フォルダ内を移動するときの順序です。「名前」では「img2」が「img10」より前になり、使用言語の文字順に従います。
settings-browsing-sort-order-natural = 名前
settings-browsing-sort-order-date-taken = 撮影日
settings-browsing-sort-order-modified = 変更日
settings-browsing-sort-order-size = ファイルサイズ
settings-browsing-sort-order-type = ファイルの種類
settings-browsing-sort-descending = 逆順
settings-browsing-sort-descending-help = 最後から最初の順に並べ替えます。
settings-browsing-loop-folder = フォルダの最後でループ
settings-browsing-loop-folder-help = 最後の画像の次は最初の画像に、最初の画像の前は最後の画像に移動します。
settings-browsing-include-subfolders = サブフォルダを含める
settings-browsing-include-subfolders-help = サブフォルダ内の画像も順に表示します。
settings-browsing-hold-acceleration = 次／前を押し続けたときの加速
settings-browsing-hold-acceleration-help = キーやボタンを押し続けている間に、画像送りがどれだけ速くなるかを指定します。
settings-browsing-hold-acceleration-off = 加速しない
settings-browsing-hold-acceleration-gentle = 緩やか
settings-browsing-hold-acceleration-normal = 標準
settings-browsing-hold-acceleration-fast = 速い
settings-browsing-confirm-trash = ゴミ箱に移動する前に確認
settings-browsing-confirm-trash-help = ゴミ箱への移動はいつでも取り消せるため、デフォルトでは { -app-name } は確認しません。
settings-browsing-arrows-follow-reading-direction = 矢印キーを読む方向に合わせる
settings-browsing-arrows-follow-reading-direction-help = 右から左に書く言語では、左矢印キーで次の画像に移動します。

## カラー

settings-colour-management = カラーマネジメント
settings-colour-management-help = 画像のカラープロファイルとディスプレイのプロファイルを使用して、本来意図された色で表示します。
settings-colour-display-profile = ディスプレイプロファイル
settings-colour-display-profile-help = ディスプレイのカラープロファイルです。「自動」では、オペレーティングシステムが報告するプロファイルを使用します。
settings-colour-rendering-intent = レンダリングインテント
settings-colour-rendering-intent-help = ディスプレイで表示できない色を、表示可能な範囲に収める方法です。
settings-colour-rendering-intent-perceptual = 知覚的
settings-colour-rendering-intent-relative-colorimetric = 相対的な色域を維持
settings-colour-rendering-intent-saturation = 彩度
settings-colour-rendering-intent-absolute-colorimetric = 絶対的な色域を維持
settings-colour-black-point-compensation = 黒点の補正
settings-colour-black-point-compensation-help = ディスプレイの黒が画像の黒より明るい場合でも、シャドウのディテールを保持します。
settings-colour-untagged-images = カラープロファイルのない画像
settings-colour-untagged-images-help = 使用する色空間が指定されていない画像の表示方法です。
settings-colour-untagged-images-srgb = sRGB として扱う
settings-colour-untagged-images-display = 変換せずに表示
settings-colour-dithering = ディザリング
settings-colour-dithering-help = 色深度を下げる際に細かいブルーノイズを加え、空やグラデーションに縞模様が出るのを防ぎます。
settings-colour-hdr-output = HDR出力
settings-colour-hdr-output-help = 対応ディスプレイでHDR画像を本来の明るさで表示します。自動では、ディスプレイが追加の明るさを報告したときにHDRを使い、それ以外では明るい部分をなめらかに圧縮します。
settings-colour-hdr-output-auto = 自動
settings-colour-hdr-output-on = オン
settings-colour-hdr-output-off = オフ
# machine-drafted, needs native review
settings-colour-ocio-config = OCIO 構成
settings-colour-ocio-config-help = EXR、Radiance HDR、PFM 画像用の OpenColorIO 構成ファイル (.ocio) です。空欄の場合は $OCIO が指す構成を使用します。
settings-colour-ocio-display = OCIO ディスプレイ
settings-colour-ocio-display-help = OCIO 構成のディスプレイのうち、お使いのモニターに対応するものです。
settings-colour-ocio-display-default = 構成の最初のディスプレイ
settings-colour-ocio-view = OCIO ビュー
settings-colour-ocio-view-help = シーンリニア (scene-linear) 画像をそのディスプレイでどう表示するかです (トーンマッピングされたビューなど)。
settings-colour-ocio-view-default = ディスプレイの最初のビュー
settings-colour-ocio-input = OCIO 入力色空間
settings-colour-ocio-input-help = 画像の値の色空間です。
settings-colour-ocio-input-default = 構成の scene-linear ロール
# machine-drafted, needs native review
settings-colour-proof-profile = ソフトプルーフのプロファイル
settings-colour-proof-profile-help = 現像のソフトプルーフで最後に使ったプリンタープロファイル。
settings-colour-proof-intent = ソフトプルーフのレンダリングインテント
settings-colour-proof-intent-help = プリンターで印刷できない色の扱い方。
settings-colour-proof-intent-perceptual = 知覚的
settings-colour-proof-intent-relative = 相対的な色域を維持
settings-colour-proof-bpc = ソフトプルーフの黒点の補正
settings-colour-proof-bpc-help = 黒をプリンターの黒に合わせます。
settings-colour-proof-paper = ソフトプルーフの紙の白
settings-colour-proof-paper-help = 紙そのものの色を表示します。
settings-colour-proof-ink = ソフトプルーフのインクの黒
settings-colour-proof-ink-help = インクの黒の暗さを表示します。

## メタデータとプライバシー

settings-metadata-show-info-on-open = 開いたときに情報パネルを表示
settings-metadata-show-info-on-open-help = 画像を開くたびに、カメラ、日付、場所の詳細を示すパネルを開きます。
settings-metadata-info-detail = 情報パネルの表示内容
settings-metadata-info-detail-help = 「概要」では最も役立つ詳細をわかりやすい言葉で説明します。「すべての項目」ではファイルに保存されているすべての情報を一覧表示します。
settings-metadata-info-detail-summary = 概要
settings-metadata-info-detail-all = すべての項目
settings-metadata-reverse-geocoding = GPS 位置の地名を表示
settings-metadata-reverse-geocoding-help = 内蔵データから最寄りの町を調べます。オフラインで動作するため、位置情報がコンピュータの外に送信されることはありません。
settings-metadata-hide-gps = 画面上で GPS 位置を隠す
settings-metadata-hide-gps-help = 画面共有中などに、座標や場所を表示しません。ファイルは変更されません。
# machine-drafted, needs native review
settings-metadata-develop-sidecar = 現像の編集を写真の横に保存
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = ファイルの横に IMG_1234.CR2.specular.xmp を書き込み、編集がファイルと一緒に移動します。オフ：編集はライブラリにのみ残ります。
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = 現像で Lightroom の設定を読み込む
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = Lightroom の設定があり編集のない写真は、その設定で近似的に開きます。オフ: 現像にボタンが出ます。
settings-metadata-units = 単位
settings-metadata-units-help = 高度、距離、速度の単位です。
settings-metadata-units-auto = 地域のフォーマットに従う
settings-metadata-units-metric = メートル法
settings-metadata-units-imperial = ヤード・ポンド法
# machine-drafted, needs native review
settings-metadata-basemap = ベースマップ
settings-metadata-basemap-help = 写真の位置の下に表示する地図です。オンライン地図は表示範囲のタイルをダウンロードします。
settings-metadata-basemap-off = 内蔵地図のみ (オフライン)
settings-metadata-basemap-openstreetmap = OpenStreetMap (オンライン)
settings-metadata-basemap-custom = その他のタイルサーバー (オンライン)
settings-metadata-basemap-file = 自分の地図ファイル
settings-metadata-basemap-url = タイルサーバーのアドレス
settings-metadata-basemap-url-help = {"{"}z{"}"}、{"{"}x{"}"}、{"{"}y{"}"} を含むアドレス。例: https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = 地図ファイル
settings-metadata-basemap-path-help = MBTiles または PMTiles ファイル。オフラインで動作します。
settings-metadata-basemap-cache = 地図タイルのキャッシュ
settings-metadata-basemap-cache-help = ダウンロードしたタイル用のディスク容量です。古いものから削除されます。
# machine-drafted, needs native review
settings-metadata-basemap-allowed = 許可したタイルサーバー
settings-metadata-basemap-allowed-help = タイルのダウンロードに同意した地図サーバーです。

## パフォーマンス

settings-performance-ram-cache = メモリキャッシュ
settings-performance-ram-cache-help = デコード済みの画像をすぐに表示できるよう保持するために使用するメモリです。「自動」では、コンピュータのメモリ容量に応じて決定します。
settings-performance-vram-cache = GPU メモリキャッシュ
settings-performance-vram-cache-help = 画像やタイルに使用する GPU メモリです。「自動」では、GPU に応じて決定します。
settings-performance-prefetch-depth = 先読みする画像
settings-performance-prefetch-depth-help = 移動している方向にある画像を何枚先までデコードしておくかを指定します。
settings-performance-decoder-threads = デコーダのスレッド
settings-performance-decoder-threads-help = 画像のデコードに使用する CPU のスレッドです。「自動」では、すべての高性能コアを使用し、ファイルに応じて調整します。
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = コンピュータに複数の GPU がある場合に使用するものを指定します。
settings-performance-gpu-adapter-high-performance = 最高のパフォーマンス
settings-performance-gpu-adapter-low-power = 最小の消費電力
settings-performance-processing = 画像を処理する場所
settings-performance-processing-help = 一部の処理は CPU、GPU、ハードウェアデコーダーのいずれでも実行できます。自動ではこのコンピュータを計測し、表示中の画像には最も速い方法を、バックグラウンドの処理には最も負荷の少ない方法を選びます。 このコンピュータで選ばれた方法とその理由は下に表示されます。
settings-performance-processing-automatic = 自動
settings-performance-processing-prefer-gpu = GPU を優先
settings-performance-processing-cpu-only = CPU のみ
settings-performance-drag-preview = ドラッグ中のプレビュー
settings-performance-drag-preview-help = スライダーをドラッグしている間の現像の表示方法。自動では、このコンピューターが追いつかないときだけ小さい画像を使います。
settings-performance-drag-preview-automatic = 自動
settings-performance-drag-preview-sharp = 常に鮮明
settings-performance-drag-preview-fast = 常に高速

## Settings → Performance → How { -app-name } runs on this computer (DESIGN.md §12.3.1)
# machine-drafted, needs native review
settings-paths-title = このコンピュータでの { -app-name } の動作
settings-paths-intro = { -app-name } はこのコンピュータで各処理の方法ごとに時間を計測し、最適な方法を使います。時間は { $mpx } メガピクセルの写真の場合です。
settings-paths-intro-measuring = 計測はまだ完了していません。{ -app-name } はコンピュータがアイドル状態のときに計測します。
settings-paths-intro-battery = バッテリー駆動中:消費電力の少ない方法が優先されます。
settings-paths-keywords = GPU CPU NPU グラフィックボード ハードウェア アクセラレーション デコーダー ベンチマーク 速度
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU(全コア)
paths-engine-gpu = GPU
paths-engine-hw-decoder = ハードウェアデコーダー
paths-engine-npu = NPU
settings-paths-your-setting = { $engine }(設定による)
settings-paths-measured-faster = このコンピュータでの計測:{ $other } の { $ratio } 倍の速さです。
settings-paths-measured-similar = このコンピュータでの計測:{ $other } とほぼ同じ速さです。
settings-paths-measured-power = バッテリー駆動中は、これが最も消費電力の少ない方法です(このコンピュータで計測)。
settings-paths-not-measured = まだ計測していません。この種類のコンピュータでの標準的な選択です。
settings-paths-set-by = 設定「{ $setting }:{ $option }」により選択されています。
settings-paths-blocked-check = { $engine } は { $date } に無効になりました。結果が CPU の結果と一致しませんでした。
settings-paths-blocked-error = { $engine } は { $date } にエラーのため無効になりました。
settings-paths-blocked-lost = { $engine } は { $date } に応答しなくなったため無効になりました。
settings-paths-blocked-undated = { $engine } はエラーのため無効になりました。
settings-paths-checking = { $engine } は、表示中の画像に使う前に CPU の結果と照合されます。
settings-paths-exploring = { $engine } をバックグラウンドで試しています。
settings-paths-unavailable = このコンピュータにはこの処理に使える { $engine } がありません。
settings-paths-background-cheaper = バックグラウンド:{ $engine }(計測で CPU 時間が { $percent } 少ない)。
settings-paths-background-power = バックグラウンド:省電力のため { $engine }。
settings-paths-background = バックグラウンド:{ $engine }。
settings-paths-detail-measured = { $engine }:{ $time } ms、CPU 時間 { $cpu } ms、{ $runs ->
   *[other] 計測 { $runs } 回
}({ $confidence })
settings-paths-detail-guess = { $engine }:約 { $time } ms(推定)
settings-paths-confidence-low = 信頼度 低
settings-paths-confidence-medium = 信頼度 中
settings-paths-confidence-high = 信頼度 高
settings-paths-machine = 計測対象:{ $gpu } · { $driver } · { $cpu }
settings-paths-retry = 再試行
settings-paths-measure-again = 計測し直す
settings-paths-measure-again-question = このコンピュータでの計測結果(無効になった方法を含む)を消去し、アイドル時に計測し直しますか?
settings-paths-measure-again-confirm = 計測結果を消去

settings-performance-display-cache = 表示用キャッシュをディスクに保持
settings-performance-display-cache-help = 画像を画面サイズに縮小したコピーを保存し、一度表示したフォルダをすぐに開けるようにします。
settings-performance-display-cache-size = 表示用キャッシュのサイズ
settings-performance-display-cache-size-help = キャッシュがいっぱいになると、古い項目から削除されます。
settings-performance-display-cache-location = 表示用キャッシュの場所
settings-performance-display-cache-location-help = 高速なドライブ上のフォルダを推奨します。
settings-performance-fast-simd = SIMD 命令を使用
settings-performance-fast-simd-help = デコードに CPU のベクトル命令を使用します。オフにするのは問題の診断時のみにしてください。
settings-performance-fast-dct-scaling = 画面サイズでデコード
settings-performance-fast-dct-scaling-help = 大きな JPEG を表示サイズで直接デコードします。オフにするのは問題の診断時のみにしてください。
settings-performance-fast-parallel-decode = 複数のコアでデコード
settings-performance-fast-parallel-decode-help = 大きな画像を複数の CPU コアに分割して処理します。オフにするのは問題の診断時のみにしてください。
settings-performance-fast-restart-index = 拡大表示用に大きな JPEG をインデックス化
settings-performance-fast-restart-index-help = JPEG の各部分の開始位置を記憶し、拡大した領域をすばやくデコードできるようにします。オフにするのは問題の診断時のみにしてください。
settings-performance-fast-yuv-upload = GPU で色を変換
settings-performance-fast-yuv-upload-help = JPEG の色を GPU または CPU で変換します。自動ではこのコンピュータで速い方を選びます。オフにするのは問題の診断時のみにしてください。
settings-performance-fast-yuv-upload-auto = 自動
settings-performance-fast-yuv-upload-on = オン
settings-performance-fast-yuv-upload-off = オフ
settings-performance-fast-async-io = 非同期ファイル読み込み
settings-performance-fast-async-io-help = オペレーティングシステムの最速のファイルインターフェイスで先読みします。オフにするのは問題の診断時のみにしてください。
settings-performance-fast-mmap = メモリマップトファイル
settings-performance-fast-mmap-help = ファイルをコピーせずに読み込みます。オフにするのは問題の診断時のみにしてください。
settings-performance-hardware-decode = ハードウェアデコード
settings-performance-hardware-decode-help = GPU 内蔵のデコーダーでJPEG写真を開きます。自動では効果がある場合、主にバックグラウンドで準備する画像に使います。最初に通常のデコーダーと照合され、対応できない画像は通常どおり開かれます。
settings-performance-hardware-decode-auto = 自動
settings-performance-hardware-decode-on = オン
settings-performance-hardware-decode-off = オフ
settings-performance-frame-time-hud = フレーム時間 HUD
settings-performance-frame-time-hud-help = デコード時間とフレーム時間、および使用中の高速処理を表示します。

## アクセシビリティ

settings-accessibility-reduce-motion = 視差効果を減らす
settings-accessibility-reduce-motion-help = 拡大・縮小、スライド、フェードのアニメーションを瞬時の切り替えに置き換えます。
settings-accessibility-reduce-motion-system = システムに従う
settings-accessibility-reduce-motion-on = オン
settings-accessibility-reduce-motion-off = オフ
settings-accessibility-increase-contrast = コントラストを上げる
settings-accessibility-increase-contrast-help = 枠線とテキストを強調し、半透明効果を減らします。
settings-accessibility-increase-contrast-system = システムに従う
settings-accessibility-increase-contrast-on = オン
settings-accessibility-increase-contrast-off = オフ
settings-accessibility-text-scale = テキストサイズ
settings-accessibility-text-scale-help = 標準サイズに対するテキストの大きさです。
settings-accessibility-ui-scale = インターフェイスの拡大率
settings-accessibility-ui-scale-help = オペレーティングシステムで設定されたディスプレイの拡大率に加えて適用される、インターフェイス全体の大きさです。
settings-accessibility-focus-ring-always = フォーカスリングを常に表示
settings-accessibility-focus-ring-always-help = マウスを使用しているときでも、キーボードフォーカスがあるコントロールを表示します。
settings-accessibility-screen-reader-verbosity = スクリーンリーダーの詳細度
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = 画像を移動したときにスクリーンリーダーが読み上げる情報量と、カーソル下の表示値をどこまで読み上げるかです。
settings-accessibility-screen-reader-verbosity-concise = 簡潔
settings-accessibility-screen-reader-verbosity-normal = 標準
settings-accessibility-screen-reader-verbosity-verbose = 詳細
# machine-drafted, needs native review
settings-accessibility-announce-cursor = カーソル下の値を読み上げ
settings-accessibility-announce-cursor-help = カーソルまたはキーボード十字線が止まると、スクリーンリーダーがその下のピクセル値、HU、温度、位置を読み上げます。

## プライバシー

# machine-drafted, needs native review
settings-privacy-no-network = ネットワークなし
# machine-drafted, needs native review
settings-privacy-no-network-help = オフ：接続するのは次のものだけで、それぞれオンにするか、それで何かを開いたときに限ります：アップデートの確認、ローカル AI のモデルのダウンロード、オンライン地図、Webアドレスやクラウドストレージの画像、追加したサーバー（DICOMweb、OMERO、地図）。オン：何も接続しません。

## 詳細

settings-advanced-portable-mode = ポータブルモード
settings-advanced-portable-mode-help = 設定とキャッシュを、ユーザプロファイルではなくプログラムと同じ場所のフォルダに保存します。
settings-advanced-log-level = ログの詳細度
settings-advanced-log-level-help = { -app-name } がログファイルに書き込む情報量です。
settings-advanced-log-level-error = エラーのみ
settings-advanced-log-level-warn = 警告
settings-advanced-log-level-info = 情報
settings-advanced-log-level-debug = デバッグ
settings-advanced-log-level-trace = すべて
settings-advanced-developer-mode = 開発者モード
settings-advanced-developer-mode-help = { -app-name } の開発とテスト用のツール（言語一覧の疑似言語など）を表示します。
# machine-drafted, needs native review
settings-advanced-audit-log = ローカル監査ログ
# machine-drafted, needs native review
settings-advanced-audit-log-help = 開いた医用画像、書き出し、匿名化、印刷、PACS への送信をデータフォルダの audit.jsonl に記録します。記録はこのコンピューターから出ません。
settings-advanced-hw-report = ハードウェアレポート…
settings-advanced-hw-report-help = { -app-name } が検出した CPU と GPU の機能、および使用している高速処理を表示します。
# machine-drafted, needs native review
settings-advanced-renderer-info = レンダラー情報…
settings-advanced-renderer-info-help = { -app-name } が使用するグラフィックアダプター、ドライバー、表示形式、メモリ制限を表示します。そのままバグ報告にコピーできます。
settings-advanced-edit-settings-file = settings.toml を編集
settings-advanced-edit-settings-file-help = { -app-name } の内蔵エディターで設定ファイルを編集します。入力中にファイルを検査し、エラーがないときだけ保存します。
settings-advanced-import = 設定を読み込む…
settings-advanced-import-help = { -app-name } から書き出したファイルから設定を読み込みます。
settings-advanced-export = 設定を書き出す…
settings-advanced-export-help = バックアップや別のコンピュータ用に、すべての設定を 1 つのファイルに保存します。
settings-advanced-reset-all = すべての設定をリセット…
settings-advanced-reset-all-help = すべての設定をデフォルトに戻します。この操作は取り消せます。

# Stage 1 merge (machine-drafted, needs native review)
settings-performance-stall-warning = 読み込みの停滞を報告するまでの時間
settings-performance-stall-warning-help = 画像がこの時間を過ぎても読み込み中の場合、その読み込み状態と保留中のジョブをログ（標準エラー出力または SPECULAR_LOG_FILE）に書き込みます。
# machine-drafted, needs native review
settings-performance-insights = パフォーマンスのヒント
settings-performance-insights-help = 同じ理由で画像を開くのが繰り返し遅くなる場合、隅に「遅い原因は？」と改善策を表示します。学習した内容はこのコンピューターから出ません。
settings-performance-insights-storage = 低速なストレージに関するヒント
settings-performance-insights-storage-help = ファイルの読み込み（ネットワーク共有、低速なドライブ、アーカイブから）によって画像を開くのが繰り返し遅れる場合にヒントを表示します。
settings-performance-insights-decoder = デコードに関するヒント
settings-performance-insights-decoder-help = ハードウェアデコーダーが役立つ可能性があるのに、画像が CPU で繰り返しデコードされている場合にヒントを表示します。
# machine-drafted, needs native review
settings-performance-insights-restore = 非表示にしたヒントを再表示する
settings-performance-insights-restore-help = 「なぜ遅いの？」で × を押して非表示にしたヒントを、画像を開くときとサムネイルの両方について元に戻します。非表示のヒントがない間は使用できません。

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = 保存
config-editor-revert = 元に戻す
config-editor-close = 閉じる
config-editor-modified = 変更あり
config-editor-text = 設定ファイル
config-editor-position = { $line } 行、{ $column } 列
config-editor-saved = settings.toml を保存して適用しました。
config-editor-cant-save = 保存できません。修正が必要な問題: { $count } 件。
config-editor-save-failed = settings.toml を保存できません: { $reason }
config-editor-conflict = settings.toml が別の場所で変更されました。
config-editor-reload = 再読み込み
config-editor-keep-mine = 自分の変更を保持
config-editor-discard-question = settings.toml の未保存の変更を破棄しますか?
config-editor-discard = 破棄
config-editor-keep-editing = 編集を続ける
config-editor-find-placeholder = ファイル内を検索
config-editor-find-count = { $current } / { $total }
config-editor-find-none = 一致なし
config-editor-find-previous = 前の一致
config-editor-find-next = 次の一致
config-editor-find-close = 検索を閉じる
config-editor-problems = 問題
config-editor-problems-count =
    { $count ->
        [0] 問題なし
       *[other] 問題: { $count } 件
    }
config-editor-no-problems = 問題はありません。ファイルを保存できます。
config-editor-problem-at = { $line } 行: { $message }
config-editor-severity-error = エラー
config-editor-severity-warning = 警告
config-editor-severity-info = 情報
config-editor-problem-syntax = 無効な TOML: { $reason }
config-editor-problem-duplicate = { $key } が同じテーブルで 2 回設定されています。どちらかを削除してください。
config-editor-problem-unknown-section = 設定セクション [{ $key }] はありません。
config-editor-problem-unknown-section-suggestion = 設定セクション [{ $key }] はありません。[{ $suggestion }] のことですか?
config-editor-problem-unknown-key = 設定 { $key } はありません。
config-editor-problem-unknown-key-suggestion = 設定 { $key } はありません。{ $suggestion } のことですか?
config-editor-problem-invalid = { $key } の値が無効です: { $reason }
config-editor-problem-migrated = { $key } は古いバージョンの { -app-name } の書き方です。読み込み時に変換されます。
config-editor-problem-newer-version = このファイルは新しいバージョンの { -app-name } で書かれています (形式 { $found }、このバージョンが読めるのは { $supported })。不明な設定は保持されます。
config-editor-problem-command = [[commands]] のコマンド { $index }: { $reason }
config-editor-problem-hook = [[hooks]] のフック { $index }: { $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = キー割り当てファイル
config-editor-keymap-saved = keymap.toml を保存して適用しました。
config-editor-keymap-save-failed = keymap.toml を保存できません: { $reason }
config-editor-keymap-conflict = keymap.toml が別の場所で変更されました。
config-editor-keymap-discard-question = keymap.toml の未保存の変更を破棄しますか？
config-editor-new-file = 新しいファイル（保存時に作成）
config-editor-no-folder = 保存先の設定フォルダーがありません。

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = すべての設定を既定値に戻しますか？
settings-reset-all-confirm-detail = 設定のすべての項目に影響します。後で元に戻せます。
settings-reset-all-confirm-ok = すべてリセット
settings-reset-all-done = すべての設定を既定値に戻しました。

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = ハードウェアエンコーダー

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = { $engine } は、使う前に CPU の結果と照合されます。
settings-paths-unit-thumbnail = 時間は { $size } のサムネイル 1 枚あたりです
settings-paths-unit-frame = 時間は { $size } のビデオフレーム 1 枚あたりです
# machine-drafted, needs native review
settings-metadata-develop-calibration = キャリブレーション画像のフォルダー
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = 現像用のフラット画像とダーク画像。ファイル名かフォルダー名に「flat」または「dark」を含めます。

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = 白
settings-colour-light-grey = 明るい灰色
settings-colour-grey = 灰色
settings-colour-dark-grey = 暗い灰色
settings-colour-black = 黒
settings-colour-yellow = 黄
settings-colour-orange = オレンジ
settings-colour-red = 赤
settings-colour-magenta = マゼンタ
settings-colour-blue = 青
settings-colour-cyan = シアン
settings-colour-green = 緑
