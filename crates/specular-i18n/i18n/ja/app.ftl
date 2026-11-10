# machine-drafted, needs native review
# Specular — ビューアのウインドウ、コマンドパレット、エラー、情報ウインドウ。日本語。

-app-name = Specular

## ツールバーのツールチップとアクセシビリティラベル

toolbar-open = 画像を開く…
toolbar-prev = 前の画像
toolbar-next = 次の画像
toolbar-back-gallery = ギャラリーに戻る
toolbar-zoom-out = 縮小
toolbar-zoom-in = 拡大
toolbar-zoom-level = 拡大率とプリセット
toolbar-zoom-fit = ウインドウに合わせる
toolbar-zoom-100 = 実際のサイズ（{ PERCENT(100) }）
toolbar-rotate-left = 左に回転
toolbar-rotate-right = 右に回転
toolbar-fullscreen-enter = フルスクリーンにする
toolbar-fullscreen-exit = フルスクリーンを終了
toolbar-info = 画像の情報
toolbar-settings = 設定
toolbar-delete = ゴミ箱に移動
toolbar-copy = 画像をコピー
toolbar-palette = コマンドパレット
toolbar-go-to = 画像へ移動…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = ツールバーをカスタマイズ…
toolbar-more = その他
toolbar-announce-moved = { $item }: { $count } 個中 { $index } 番目
toolbar-announce-added = { $item }を追加しました: { $count } 個中 { $index } 番目
toolbar-announce-removed = { $item }をツールバーから削除しました
toolbar-announce-reset = ツールバーを既定に戻しました
tooltip-with-shortcut = { $label }（{ $shortcut }）

## 表示

readout-position = { $index } / { $count }
readout-position-a11y = { $count } 枚中 { $index } 枚目の画像
readout-zoom = { $zoom }
readout-zoom-a11y = 拡大率 { $zoom }
readout-zoom-fit = ウインドウに合わせる（{ $zoom }）
zoom-preset-fit = ウインドウに合わせる
zoom-preset-fill = ウインドウを埋める
zoom-preset-actual = 実際のサイズ
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = フルスクリーン
zoom-preset-percent = { $zoom }
readout-dimensions = { $width } × { $height }
status-image-count =
    { $count ->
       *[other] { $count } 枚の画像
    }

## コマンドパレット

palette-placeholder = コマンドまたは設定を入力…
palette-no-results = 一致するコマンドはありません
palette-set-setting = 設定：{ $setting } → { $value }
palette-next = 次の画像
palette-prev = 前の画像
palette-first = 最初の画像
palette-last = 最後の画像
palette-go-to = 画像へ移動…
palette-zoom-in = 拡大
palette-zoom-out = 縮小
palette-zoom-fit = ウインドウに合わせて表示
palette-zoom-fill = ウインドウを埋めて表示
# machine-drafted, needs native review
palette-zoom-fit-width = 幅に合わせて表示
palette-zoom-fit-height = 高さに合わせて表示
palette-zoom-100 = { PERCENT(100) } で表示
palette-zoom-200 = { PERCENT(200) } で表示
palette-zoom-300 = { PERCENT(300) } で表示
palette-zoom-400 = { PERCENT(400) } で表示
palette-zoom-rectangle = 選択範囲を拡大
palette-rotate-left = 左に回転
palette-rotate-right = 右に回転
palette-flip-horizontal = 左右反転
palette-flip-vertical = 上下反転
palette-toggle-fullscreen = フルスクリーンの切り替え
palette-navigator = ナビゲーターの表示／非表示
palette-loupe = ルーペ
palette-pixel-grid = ピクセルグリッドの表示／非表示
palette-info-panel = 情報パネルの表示／非表示
palette-copy-image = 画像をコピー
# machine-drafted, needs native review
palette-read-codes = コードを読み取る
# machine-drafted, needs native review
palette-find-text = テキストを探す
# machine-drafted, needs native review
palette-face-boxes = 顔を表示
palette-copy-path = ファイルパスをコピー
palette-move-to-trash = ゴミ箱に移動
palette-undo-trash = ゴミ箱への移動を取り消す
palette-open = 画像を開く…
# machine-drafted, needs native review
palette-open-address = アドレスを開く…
palette-settings = 設定
# machine-drafted, needs native review
palette-customise-toolbar = ツールバーをカスタマイズ…
palette-cheat-sheet = キーボードショートカット
palette-hud = フレーム時間 HUD の表示／非表示
palette-language = 言語
palette-theme = テーマ
# machine-drafted, needs native review
palette-interface-larger = インターフェースを大きく
palette-interface-smaller = インターフェースを小さく
palette-interface-reset = インターフェースを{ PERCENT(100) }に
palette-quit = { -app-name } を終了

## ショートカット一覧

cheatsheet-title = キーボードショートカット
cheatsheet-navigation = ナビゲーション
cheatsheet-zoom-pan = 拡大・縮小とスクロール
cheatsheet-view = 表示
cheatsheet-file = ファイル
cheatsheet-window = ウインドウ
cheatsheet-help = ヘルプ
cheatsheet-hold = { $key } を押し続ける
cheatsheet-close-hint = { $key } または Esc キーで閉じます

## エラー

# machine-drafted, needs native review
viewer-no-preview = このファイルにはプレビューがありません。
error-cannot-open = { $file } を開けません
error-cannot-open-reason = { $file } を開けません：{ $reason }
error-unsupported-format = { $file } は、{ -app-name } がまだ読み込めないフォーマットです。
error-file-truncated = { $file } は不完全です。読み込めた部分を表示しています。
error-file-too-large = { $file } は大きすぎるため開けません（{ $megapixels } メガピクセル。上限は { $limit } メガピクセルです）。
error-not-found = { $file } はもう存在しません。
error-permission-denied = { $file } を読み込む権限がありません。
error-details = 詳細
crash-last-time = 前回 { -app-name } が予期せず終了しました。レポートを { $path } に保存しました
error-trash-failed = { $file } をゴミ箱に移動できませんでした：{ $reason }
# machine-drafted, needs native review
# 「FILE を開けません：」などの後に表示される理由。何が起きたか、役立つ場合はどうすればよいかを述べます。
error-reason-not-found = もう存在しません。移動、名前の変更、または削除された可能性があります。
error-reason-permission = 読み込む権限がありません。所有者と権限を確認してください。
error-reason-disk-full = ディスクがいっぱいです。空き容量を確保してもう一度お試しください。
error-reason-read-only = ディスクは読み取り専用です。
error-reason-truncated = ファイルが不完全です。まだコピー中またはダウンロード中の可能性があります。
error-reason-is-folder = ファイルではなくフォルダーです。
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = サーバーがアクセスを拒否しました。データが非公開の可能性があります。S3 バケットの場合は、先に AWS 認証情報を設定してください。
error-reason-remote-missing = そのアドレスに Zarr データがありません。
# $status is an HTTP status code ("500").
error-reason-remote-status = サーバーがエラー { $status } を返しました。
error-reason-offline = サーバーに接続できません。アドレスと接続を確認してください。
# machine-drafted, needs native review
error-reason-network-off = 設定 › プライバシーでネットワークがオフになっています。
error-reason-timed-out = ドライブまたはサーバーが時間内に応答しませんでした。接続を確認してもう一度お試しください。
error-reason-memory = メモリが不足しています。ほかの画像やアプリケーションを閉じてもう一度お試しください。
error-reason-unsupported = { -app-name } がまだ読み込めないフォーマットです。
# $detail はデコーダー自身の英語の技術的な説明です（"12-bit lossless JPEG"）。
error-reason-unsupported-detail = { -app-name } がまだ読み込めない機能を使用しています（{ $detail }）。
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = { -app-name } が読み込める .{ $ext } ファイル（{ $formats }）のいずれでもありません。
error-reason-damaged = ファイルが壊れているか、名前が示す形式ではありません（{ $detail }）。
error-reason-too-large = 大きすぎるため開けません。
# machine-drafted, needs native review
error-reason-needs-memory = { $needed } の空き RAM が必要ですが、現在の空きは { $free } だけです。ほかのアプリをいくつか閉じてもう一度お試しいただくか、メモリの多いコンピュータで開いてください。
error-needs-memory = この画像には { $needed } の空き RAM が必要ですが、現在の空きは { $free } だけです。ほかのアプリをいくつか閉じてもう一度お試しいただくか、メモリの多いコンピュータで開いてください。
error-needs-memory-short = { $needed } の空き RAM が必要
error-reason-cancelled = 開く処理が中止されました。
error-reason-trash-access = もう存在しないか、変更する権限がありません。
error-reason-restore-collision = 同じ名前のファイルが元のフォルダーにすでにあります。
error-reason-trash-root = ドライブ全体をゴミ箱に移動することはできません。
error-reason-not-in-trash = もうゴミ箱にありません。
error-reason-program-missing = プログラムがインストールされていないか、移動されています。もう一度選ぶか、再インストールしてください。
error-reason-program-not-allowed = プログラムの実行が許可されていません。実行可能になっているか確認してください。
# machine-drafted, needs native review
error-reason-permission-write = 変更する権限がありません。所有者と権限を確認してください。
error-reason-file-has-errors = エラーがあります。内容が失われないよう、先に修正してください。
error-reason-not-dicom = DICOM ファイルではありません。
error-reason-no-image = 画像が含まれていません。
error-reason-needs-layout = ヘッダーのないボリュームです。開くには、サイズ、サンプルの型、バイト順が必要です。
error-reason-not-archive = { -app-name } が読み込めるアーカイブではありません。
error-reason-needs-sandbox = この種類のアーカイブは { -app-name } の保護されたリーダーでのみ開けますが、利用できません。
error-reason-encrypted = 暗号化されているため、パスワードが必要です。
error-reason-wrong-password = パスワードが間違っています。
error-reason-checksum = ファイルが壊れています。内容がチェックサムと一致しません。
error-reason-missing-entry = 中のファイルが見つかりません。
error-reason-busy = 別のプログラムが使用中です。そちらで閉じてからもう一度お試しください。
error-reason-catalogue-damaged = カタログファイルが壊れています。
error-reason-cannot-open = ファイルを開けません。フォルダーが存在し、書き込めることを確認してください。
error-reason-newer-version = 新しいバージョンの { -app-name } で書き込まれています。変更するには { -app-name } を更新してください。
error-reason-internal = { -app-name } の内部で問題が発生しました。詳細はログを確認してください。
error-reason-volume-too-few = { $count ->
   *[other] { $count } 枚のスライスではボリュームを作成するのに足りません。
}
error-reason-volume-no-geometry = 位置や向きが記録されていないスライスがあります。
error-reason-volume-not-parallel = スライスが平行ではありません。
error-reason-volume-shared-position = { $count } か所の位置それぞれを複数の画像が共有しています (位相、エコー、または時系列)。
error-reason-volume-missing = 約 { $count } 枚のスライスが欠けています。
error-reason-volume-mismatch = スライスのサイズまたはピクセル間隔が異なります。
error-reason-volume-not-grey = スライスがグレー値ではなくカラーです。
error-reason-volume-skewed = ボリュームの軸が直交していません。
error-reason-volume-too-large = { $need } MiB 必要ですが、許可されているのは { $budget } MiB だけです。
error-reason-url-not-specular = { -app-name } のリンクではありません。
error-reason-url-too-long = 長すぎます。
error-reason-url-damaged = 壊れています。コピー時に途中で切れたか、変更された可能性があります。
error-reason-url-unknown = このバージョンの { -app-name } が知らない内容を要求しています。{ -app-name } を更新すると解決する場合があります。
error-reason-url-not-absolute = ファイルパスはフルパスで指定する必要があります。
error-reason-url-no-paths = ファイルが指定されていません。
error-reason-url-too-many = 指定されたファイルが多すぎます。

## 通知

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = 完了できませんでした：{ $reason }
toast-moved-to-trash = ゴミ箱に移動しました
toast-moved-to-trash-named = { $file } をゴミ箱に移動しました
toast-moved-to-trash-count =
    { $count ->
       *[other] { $count } 枚の画像をゴミ箱に移動しました
    }
toast-undo = 取り消す
toast-restored = { $file } を元に戻しました
toast-copied-image = 画像をコピーしました
toast-copied-path = パスをコピーしました
toast-copied-report = レポートをコピーしました
toast-copied-value = 値をコピーしました
toast-loop-start = 最初の画像に戻りました
toast-loop-end = 最後の画像に移動しました
# machine-drafted, needs native review
toast-interface-size = インターフェースのサイズ { PERCENT($percent) }
toast-language-changed = 言語を { $language } に変更しました

## 空の状態

empty-drop = ここに画像をドロップするか、{ $shortcut } を押してください
empty-open-button = 画像を開く…
empty-drop-release = 離すと開きます
empty-folder = このフォルダには { -app-name } で開ける画像がありません。
# machine-drafted, needs native review
# 空のウインドウの「画像を開く…」の下：ほかの機能の見つけ方。
empty-hint-keys = { $cheatsheet } でキーボードショートカット、{ $palette } で任意のコマンドを検索できます。
empty-hint-cheatsheet = { $cheatsheet } でキーボードショートカットを表示します。
empty-hint-palette = { $palette } で任意のコマンドを検索できます。
# ファイルを開くのに時間がかかるとき、スピナーの下に表示。
viewer-opening = { $file } を開いています…
fullscreen-exit-hint = Esc キーでフルスクリーンを終了します

## 言語の選択

language-picker-title = 言語
# machine-drafted, needs native review
language-picker-search = 言語を検索
language-completeness = { $percent } 翻訳済み
language-system-default = システムのデフォルト（{ $language }）
language-pseudo-accented = 疑似ロケール：アクセント付き・長め
language-pseudo-rtl = 疑似ロケール：右から左に反転

## 情報ウインドウ

about-title = { -app-name } について
about-version = バージョン { $version }
about-tagline = 高速で忠実な画像ビューア。
about-copyright = © { $year } { -app-name } の作者
about-license = MIT License / Apache License 2.0 の下でライセンスされています。
about-geonames = 地名データは GeoNames（geonames.org）によるもので、Creative Commons Attribution 4.0 の下でライセンスされています。
about-unicode = ロケールデータは ICU4X 経由で Unicode CLDR から取得しています。© Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = レンズプロファイルは Lensfun データベース（lensfun.github.io）によるもので、Creative Commons Attribution-ShareAlike 3.0 の下でライセンスされています。
about-third-party = サードパーティのライセンス
about-copy-info = システム情報をコピー
about-close = 閉じる

## ビューアの画面（ステージ 1）
# machine-drafted, needs native review
app-close = 閉じる
app-truncated-badge = 不完全なファイル
app-info-summary = 概要
app-info-all-fields = すべての項目
app-info-copy-value = 値をコピー
app-info-copy-field = { $field } の値をコピー
app-info-loading = メタデータを読み込み中…
# machine-drafted, needs native review
# メタデータがピクセル以外に何もないファイルの「すべてのフィールド」。
app-info-no-fields = このファイルにはメタデータのフィールドがありません。
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = HDRゲインマップ
app-info-gain-map-headroom = SDRの最大{ $headroom }倍の明るさ
app-copy-failed = コピーできませんでした：{ $reason }
app-restore-failed = { $file } を元に戻せませんでした：{ $reason }
app-open-dialog-title = 画像を開く
app-open-dialog-filter = 対応ファイル
app-open-dialog-all = すべてのファイル
app-open-group-images = 画像
app-open-group-raw = カメラRAW
app-open-group-video = 動画
app-open-group-sound = 音声
app-open-group-documents = 文書
app-open-group-drawings = 図面とCAD
app-open-group-models = 3Dモデル
app-open-group-maps = 地図とトラック
app-open-group-science = 科学・医療
app-open-group-fonts = フォント
app-open-group-other = その他

## レンダラー情報（バージョン情報、設定 › 詳細）

# machine-drafted, needs native review
renderer-title = レンダラー情報
renderer-section-adapter = グラフィックアダプター
renderer-section-limits = 制限と機能
renderer-section-display = ディスプレイ
renderer-section-memory = メモリ
renderer-section-decoders = オプションのデコーダー
renderer-unknown = 不明
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = ベンダー ID とデバイス ID
# Wayland, X11, Windows or macOS.
renderer-display-server = ディスプレイサーバー
renderer-features = 使用中の機能
renderer-surface-format = ウィンドウの形式
renderer-surface-formats = 利用可能な形式
renderer-present-mode = 表示モード
renderer-present-modes = 利用可能なモード
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = オン（ヘッドルーム { $headroom }×）
renderer-hdr-off = オフ（ディスプレイは HDR を表示可能）
renderer-hdr-unavailable = このディスプレイでは利用できません
renderer-decode-budget = デコードに使用できる量
renderer-decode-budget-detail = 空きメモリの { PERCENT(80) }。開いているすべての画像で共有されます。
renderer-display-cache = 表示キャッシュ

## パフォーマンス HUD（F12）
# machine-drafted, needs native review
app-hud-scale = 倍率
app-hud-level = レベル
app-hud-level-thumbnail = 埋め込みサムネイル
app-hud-level-full = フル解像度
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = オープン
app-hud-decode = デコード
app-hud-upload = アップロード
app-hud-frame = フレーム
app-hud-threads = スレッド
app-hud-threads-value = { $threads }（待機中 { $queued }）
app-hud-tiles = タイル
app-hud-tiles-value = { $tiles } / { $capacity }
app-hud-ram = メモリ
app-hud-vram = GPU メモリ
app-hud-gpu = GPU

# Trash confirmation (machine-drafted, needs native review)
app-confirm-trash-title = { $file } をごみ箱に移動しますか？
# machine-drafted, needs native review
app-confirm-trash-sidecars = サイドカーもゴミ箱に移します: { $files }
app-cancel = キャンセル

# Stage 1 merge (machine-drafted, needs native review)
disabled-no-image = 画像が開かれていません
disabled-still-loading = 画像を読み込み中です
disabled-decode-failed = この画像はデコードできませんでした
disabled-no-other-images = このフォルダーに他の画像はありません
tooltip-unavailable = { $label }（使用不可: { $reason }）
error-decoder-stopped = デコーダーが画像を生成せずに停止しました
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = 同じフォルダーに脳表面 (頂点数 { $vertices } の .surf.gii ファイル) が必要です
fullscreen-back-hint = Esc キーでズームをリセット、または全画面表示を終了
action-back = パネルを閉じる(ダイアログでは先に 1 つ戻る)、ズームをリセット、全画面表示を終了、ギャラリーでは 1 つ上のフォルダーへ
about-third-party-file = { -app-name } には、それぞれのライセンスに基づくサードパーティのソフトウェアとデータが含まれています。{ -app-name } と共にインストールされる THIRD-PARTY.md を参照してください。
toast-low-memory = この画像には約 { $need } のメモリが必要な可能性がありますが、使用可能なのは { $available } だけです。読み込みが遅くなるか、失敗するおそれがあります。

hw-report-title = ハードウェアレポート
hw-report-copy = レポートをコピー
hw-report-version = バージョン
hw-report-cpu = CPU
hw-report-cores = コア
hw-report-cores-value = 論理 { $logical }、物理 { $physical }
hw-report-performance-cores = 高性能コア
hw-report-cpu-features = CPU 高速化
hw-report-none = なし
hw-report-memory = メモリ
hw-report-memory-value = 合計 { $total }、空き { $available }
hw-report-graphics-api = グラフィックス API
hw-report-driver = ドライバー
hw-report-max-texture = 最大テクスチャ
hw-report-pixels = { $width } × { $height } px
hw-report-decode-threads = デコードスレッド
hw-report-ram-cache = 画像メモリキャッシュ
hw-report-vram-cache = GPU メモリキャッシュ
hw-report-processing = 処理
hw-report-path-jpeg-decode = JPEG デコード
hw-report-path-ycbcr-to-rgb = 色変換
hw-report-path-png-unfilter = PNG デコード
hw-report-path-heic-decode = HEIC デコード
hw-report-path-inference = 画像解析
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU（並列）
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = ハードウェアデコーダー
hw-report-engine-npu = NPU
# machine-drafted, needs native review
hw-report-build-debug = デバッグビルド
hw-report-build-release = リリースビルド

## Accessibility (screen readers)
# The image as a screen reader announces it: file name and size in pixels.
a11y-image = { $name }、{ $width } × { $height } ピクセル
# The region where short notifications (toasts) appear.
a11y-notifications = 通知
# machine-drafted, needs native review
a11y-transport-playback = 再生
a11y-transport-pages = ページ
a11y-settings-sections = セクション

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = この画像のコード
codes-looking = コードを探しています…
codes-none = この画像にコードは見つかりませんでした。
codes-failed = 画像を読み取れませんでした：{ $reason }
codes-copy = 「{ $text }」をコピー
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = { $host } のリンクを開く…
codes-network-off = ネットワークがオンになっていないため、リンクは開きません。
codes-open-title = { $host } へのリンクを開きますか？
codes-open-detail = ウェブブラウザーで開きます。信頼できるリンクだけを開いてください。
codes-open = ブラウザーで開く
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = この画像のテキスト
# machine-drafted, needs native review
ocr-looking = テキストを読み取っています…
# machine-drafted, needs native review
ocr-none = この画像にテキストは見つかりませんでした。
# machine-drafted, needs native review
ocr-failed = テキストを読み取れませんでした：{ $reason }
# machine-drafted, needs native review
ocr-needs-pack = 「テキストを探す」には、ローカル AI パックのテキストモデルが必要です（設定 › ローカル AI）。
# machine-drafted, needs native review
ocr-loading = テキストモデルを読み込んでいます…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = 「{ $text }」をコピー
# machine-drafted, needs native review
ocr-copy-all = すべてのテキストをコピー
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = 確信度 { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = システム
hw-report-section-gpu = GPU
hw-report-section-decoding = デコード
hw-report-operating-system = オペレーティングシステム
hw-report-adapter = アダプター
hw-report-gpu-type = 種類
hw-report-gpu-init = 起動時間
hw-report-max-layers = テクスチャレイヤー数
hw-report-compression = テクスチャ圧縮
hw-report-unified-memory = CPU とメモリを共有
hw-report-display = ディスプレイ
hw-report-hw-decode = ハードウェアデコード
hw-report-core-cluster = コアグループ
hw-report-power = 電源
hw-report-calibration = 速度計測
hw-report-codec-others = その他
hw-report-performance-cores-value = { $total } 個中 { $count } 個
hw-report-cluster-value = { $name }:物理 { $physical }、論理 { $logical }
hw-report-thermal-nominal = 通常の温度
hw-report-thermal-fair = やや高温
hw-report-thermal-serious = 高温(速度を低下中)
hw-report-thermal-critical = 過熱
hw-report-power-battery = バッテリー駆動({ $percent })
hw-report-power-battery-unknown = バッテリー駆動
hw-report-power-mains = 電源に接続
hw-report-power-mains-percent = 電源に接続({ $percent })
hw-report-power-saving = 省電力モード オン
hw-report-gpu-discrete = 専用
hw-report-gpu-integrated = 内蔵
hw-report-gpu-virtual = 仮想
hw-report-gpu-software = ソフトウェア(CPU 上)
hw-report-gpu-other = その他
hw-report-gpu-unavailable = 利用不可
hw-report-ms = { $ms } ms
hw-report-yes = はい
hw-report-no = いいえ
hw-report-unified-platform-differs = システムの報告とは異なります
hw-report-display-value = { $fps } Hz、HDR ヘッドルーム 現在 { $now }、最大 { $max }
hw-report-hw-decode-value = { $device } の { $api }
hw-report-hw-decode-checking = 確認中…
hw-report-codec-up-to = 最大 { $width } × { $height }
hw-report-codec-none = 利用不可
hw-report-codec-h264-encode = H.264 エンコード
hw-report-supported = 対応:{ $list }
hw-report-unsupported = 非対応:{ $list }
hw-report-calibration-done = このコンピュータで計測済み
hw-report-calibration-pending = 未計測
hw-report-paths-power-saving = 省電力中:消費電力の少ない方法が優先されます
hw-report-paths-caption = { $mpx } メガピクセルの画像での処理方法
hw-report-col-task = 処理
hw-report-col-path = 方法
hw-report-col-time = 時間
hw-report-col-cpu = CPU 時間（全スレッド合計）
hw-report-col-runs = 計測回数
hw-report-col-use = 用途
hw-report-runs-untested = 未テスト
hw-report-use-viewing = 表示({ $reason })
hw-report-use-background = バックグラウンド({ $reason })
hw-report-reason-only = 唯一の方法
hw-report-reason-faster = 最速
hw-report-reason-cheaper = CPU 時間が最少
hw-report-reason-lower-power = 消費電力が最少
hw-report-reason-explore = 試行中
hw-report-reason-forced = 設定で指定
hw-report-reason-prefer-gpu = GPU を優先
hw-report-reason-cpu-only = CPU のみ
hw-report-reason-calibrate = 計測中
hw-report-reason-scaled = 縮小サイズ
hw-report-trust-unverified = 未確認
hw-report-trust-blocked = エラーのため無効
hw-report-trust-absent = このコンピュータにはありません

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = ギャラリー
cheatsheet-mode-compare = 比較
cheatsheet-gallery-browse = 閲覧
cheatsheet-gallery-select = 選択
cheatsheet-gallery-mark = 評価とフラグ
cheatsheet-gallery-move = 画像間を移動
cheatsheet-gallery-page = 前後のページへ
cheatsheet-gallery-ends = 最初または最後の画像
cheatsheet-gallery-up = 1 つ上のフォルダーへ
cheatsheet-gallery-history-back = 前のフォルダーに戻る
cheatsheet-gallery-history-forward = 次のフォルダーに進む
cheatsheet-gallery-back = 手前のビューを閉じてから戻る
cheatsheet-gallery-viewer = ビューアーに戻る
cheatsheet-gallery-album-add = 最後に使ったアルバムに追加
cheatsheet-gallery-select-one = 選択/選択解除
cheatsheet-gallery-quick-look = クイックルック
cheatsheet-gallery-filter = フィルター
cheatsheet-gallery-bigger = サムネイルを大きく
cheatsheet-gallery-smaller = サムネイルを小さく
cheatsheet-gallery-default-size = サムネイルを標準の大きさに
cheatsheet-gallery-refresh = フォルダーを更新
cheatsheet-gallery-menu = コンテキストメニュー
cheatsheet-compare-views = 比較
cheatsheet-compare-overlay = オーバーレイまたは左右に並べる
cheatsheet-compare-difference = 差分 (もう一度押すと次の種類)
cheatsheet-compare-step-down = ブレンドまたは差分ゲインを下げる
cheatsheet-compare-step-up = ブレンドまたは差分ゲインを上げる
cheatsheet-compare-pick = キー操作の対象画像
cheatsheet-compare-close = 比較を閉じる
cheatsheet-channels = チャンネル
cheatsheet-medical-window = ウィンドウ/レベル
cheatsheet-medical-measure = 計測と注釈
cheatsheet-medical-frames = フレームとシリーズ
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D と再構成
cheatsheet-medical-analysis = セグメンテーション、融合、PET
# machine-drafted, needs native review
cheatsheet-medical-studies = 検査と共有
cheatsheet-palette-only-title = コマンドパレット内（{ $key }）
cheatsheet-palette-only-title-nokey = コマンドパレット内
cheatsheet-palette-only-tag = パレット
cheatsheet-palette-only-line = { $action }: キーなし。コマンドパレットから実行（{ $key }）
cheatsheet-palette-only-line-nokey = { $action }: キーなし。コマンドパレットから実行
# チートシートでの Esc（パレットの長い説明は収まらないため）。
cheatsheet-back = 閉じる、または戻る

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = ビデオのエクスポート
hw-report-engine-hw-encoder = ハードウェアエンコーダー

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task }（{ $size } のサムネイル 1 枚あたり）
hw-report-task-frame = { $task }（{ $size } のフレーム 1 枚あたり）

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = ソフトウェアのみ

about-natural-earth = 地図データ: Natural Earth (naturalearthdata.com)、パブリックドメイン。
app-info-show-on-map = 地図に表示
