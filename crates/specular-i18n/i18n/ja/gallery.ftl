# machine-drafted, needs native review
# Specular — ギャラリー（サムネイル表示、DESIGN §10）と 設定 → ギャラリー。日本語。

## パレットとメニュー

palette-gallery = ギャラリー
gallery-palette-view = ギャラリーの表示…
gallery-palette-layout = レイアウト
gallery-palette-sort = 並べ替え
gallery-palette-group = グループ化
gallery-palette-filter = フィルター…
gallery-palette-quick-filters = クイックフィルター
gallery-palette-details = 詳細表示
gallery-palette-info-overlay = サムネイルに情報を表示
gallery-palette-stacks = RAW+JPEG と連写をまとめる
gallery-palette-subfolders = サブフォルダを含める
gallery-palette-compare = 選択した画像を比較
gallery-palette-rate = 評価…
gallery-palette-settings = ギャラリーの設定…
gallery-view-menu = 表示
gallery-more-menu = その他
gallery-open = 開く
gallery-new-window = 新しいウィンドウで開く
gallery-reveal = フォルダに表示
gallery-properties = プロパティ
gallery-copy-paths = パスをコピー
gallery-select-all = すべて選択
gallery-select-none = 選択を解除
gallery-select-invert = 選択を反転
gallery-next-unrated = 次の未評価
gallery-stack-toggle = スタックを展開/折りたたむ
gallery-run-on = { $name }（{ $count } 個のファイル）
gallery-depth-off = このフォルダのみ
gallery-depth-all = すべてのサブフォルダ
gallery-depth-levels = サブフォルダ { $depth } 階層まで

## 状態と空の表示

gallery-items = { $count } 項目
gallery-selected = { $total } 項目中 { $count } 項目を選択
gallery-shown-of = { $total } 項目中 { $count } 項目を表示
gallery-listing = フォルダを読み込み中… { $count }
gallery-empty = このフォルダには画像がありません。
gallery-empty-filter = フィルターに一致する画像はありません。
gallery-list-error = このフォルダを読み込めません: { $reason }
gallery-no-folder = フォルダが開かれていません
gallery-kind-folder = フォルダ
gallery-drag-count = { $count } 個のファイル
gallery-badge-pages = { $count } ページ

## フィルターバー

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = 名前、または rating>=3 camera:X-T5 …
gallery-filter-clear = フィルターをクリア
gallery-filter-rating = ★ { $stars } 以上
gallery-filter-label = ラベル: { $label }
gallery-filter-picks = 採用
gallery-filter-hide-rejects = 不採用を隠す
gallery-filter-unrated = 未評価
gallery-filter-raw = RAW
gallery-filter-animated = アニメーション
gallery-filter-hdr = HDR
gallery-filter-landscape = 横長
gallery-filter-portrait = 縦長

## Query errors and facets

gallery-query-unknown-field = 不明なフィールド「{ $field }」
gallery-query-unknown-field-suggest = 不明なフィールド「{ $field }」：「{ $suggestion }」のことですか？
gallery-query-missing-value = 「{ $field }」には値が必要です
gallery-query-bad-number = 「{ $value }」は数値ではありません（{ $field }）
gallery-query-bad-date = 「{ $value }」は日付ではありません（{ $field }）：2024、2024-05、2024-05-12 の形式で指定してください
gallery-query-bad-word = 「{ $value }」は { $field } に使えません：{ $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = 「{ $value }」は Exif タグ名ではありません
gallery-query-no-compare = { $field } はここでは < や > で比較できません
gallery-query-unterminated = 閉じ引用符がありません
gallery-query-unavailable = 「{ $field }」にはライブラリが必要です（ここでは使えません）
gallery-facets = ファセット
gallery-facets-counting = 集計中…
gallery-facets-empty = 集計するものがありません
gallery-facets-search = ファセットを検索
gallery-facets-no-match = 一致する値はありません
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = スタート画面
gallery-facet-format = 形式
gallery-facet-camera = カメラ
gallery-facet-lens = レンズ
gallery-facet-rating = レーティング
gallery-facet-year = 年
gallery-facet-orientation = 向き
gallery-facet-label = ラベル
gallery-facet-properties = プロパティ
gallery-facet-square = 正方形
gallery-facet-alpha = 透明度
gallery-facet-gps = 位置情報
gallery-facet-multipage = 複数ページ
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
   *[other] { $count } ファイル
}

## グループ

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start }～{ $end }
gallery-group-event-place = { $place }・{ $dates }
gallery-group-this-folder = このフォルダ
gallery-group-unknown = 不明
gallery-group-unrated = 未評価

## 選別

gallery-stars = 星 { $stars } つ
gallery-rate-none = 評価なし
gallery-pick = 採用
gallery-reject = 不採用
gallery-unflag = フラグなし
gallery-label-red = 赤
gallery-label-yellow = 黄
gallery-label-green = 緑
gallery-label-blue = 青
gallery-label-none = ラベルなし
gallery-compare-hint = 比較するには画像を 2～4 枚選択してください。

## ファイル

gallery-trash-count = { $count } 個のファイルをゴミ箱に移動
gallery-moved-to = { $count } 個のファイルを { $folder } に移動しました
gallery-copied-to = { $count } 個のファイルを { $folder } にコピーしました
gallery-move-failed = { $file } を移動できませんでした: { $reason }
gallery-drop-move = { $folder } に移動
gallery-drop-copy = { $folder } にコピー
gallery-move-here = 選択した項目をここに移動
gallery-copy-here = 選択した項目をここにコピー
gallery-not-an-image = { $file } は { -app-name } で開ける画像ではありません。
gallery-cache-cleared = サムネイルキャッシュを消去しました
# machine-drafted, needs native review
gallery-thumb-failed = サムネイルなし: { $reason }
gallery-cache-clear-failed = サムネイルキャッシュを消去できませんでした: { $reason }

## プロパティ

gallery-prop-name = 名前
gallery-prop-folder = フォルダ
gallery-prop-size = ファイルサイズ
gallery-prop-dimensions = サイズ
gallery-prop-format = 形式
gallery-prop-date = 日付
gallery-prop-camera = カメラ
gallery-prop-lens = レンズ
gallery-prop-rating = 評価
gallery-prop-label = ラベル
gallery-prop-items = 項目

## スクリーンリーダー

gallery-a11y-list = { $folder }、{ $count } 項目
gallery-a11y-dims = { $width } × { $height }
gallery-a11y-folder = フォルダ
gallery-a11y-up = { $folder } に移動しました
gallery-a11y-archive = アーカイブ
gallery-a11y-stack = { $count } 枚のスタック
gallery-a11y-filter-bar = フィルター
gallery-a11y-filter-field = 名前またはフィールドで絞り込み
gallery-a11y-chip = フィルター条件 { $term }
gallery-a11y-chip-remove = 押すと削除します
gallery-a11y-results = { $count ->
    [0] 一致する項目はありません
   *[other] { $count } 項目が一致
}

## 設定 → ギャラリー

settings-section-gallery = ギャラリー
settings-gallery-layout = レイアウト
settings-gallery-layout-help = 「正方形」はサムネイルを格子状に切り抜き、「行揃え」は各画像の形を保ち、「切り抜きなし」は画像全体を同じ大きさの枠に収めます。
settings-gallery-layout-squares = 正方形
settings-gallery-layout-justified = 行揃え
settings-gallery-layout-uncropped = 切り抜きなし
settings-gallery-layout-details = 詳細
settings-gallery-thumbnail-size = サムネイルの大きさ
settings-gallery-thumbnail-size-help = ギャラリーでは Ctrl+マウスホイール、ピンチ、Ctrl+プラス/マイナスでも変更できます。
settings-gallery-captions = ファイル名を表示
settings-gallery-captions-help = 各サムネイルの下に名前を表示します。
settings-gallery-badges = バッジを表示
settings-gallery-badges-help = RAW、アニメーション、HDR、複数ページ、評価、ラベルを示す小さな印です。
settings-gallery-sort-order = 並べ替え
settings-gallery-sort-order-help = ギャラリーの順序です。フォルダは常に先頭に表示されます。
settings-gallery-sort-order-name = 名前
settings-gallery-sort-order-date-taken = 撮影日
settings-gallery-sort-order-modified = 変更日
settings-gallery-sort-order-size = ファイルサイズ
settings-gallery-sort-order-type = ファイルの種類
settings-gallery-sort-order-dimensions = サイズ（ピクセル）
settings-gallery-sort-descending = 逆順
settings-gallery-sort-descending-help = 最後から最初の順に並べます。
settings-gallery-group-by = グループ化
settings-gallery-group-by-help = スクロール中も上部に残る見出しで区切ります。
settings-gallery-group-by-none = なし
settings-gallery-group-by-day = 日
settings-gallery-group-by-month = 月
settings-gallery-group-by-year = 年
settings-gallery-group-by-event = イベント
settings-gallery-group-by-folder = フォルダ
settings-gallery-group-by-camera = カメラ
settings-gallery-group-by-format = 形式
settings-gallery-group-by-rating = 評価
settings-gallery-group-by-target = 対象
settings-gallery-group-by-filter = フィルター
settings-gallery-per-folder-view = フォルダごとに表示を記憶
settings-gallery-per-folder-view-help = レイアウト、大きさ、並べ替え、グループ化をフォルダごとに保存します。オフにするとすべてのフォルダで同じ表示を使います。
settings-gallery-open-folders-in-gallery = フォルダをギャラリーで開く
settings-gallery-open-folders-in-gallery-help = フォルダを開いたとき、最初の画像ではなくサムネイル一覧から始めます。
settings-gallery-escape-opens-gallery = Esc でギャラリーを開く
settings-gallery-escape-opens-gallery-help = 閉じるものが何もないとき、ビューアーで Esc を押すとギャラリーに移動します。
settings-gallery-wrap-around = 矢印キーで端から反対側へ移動
settings-gallery-wrap-around-help = 一覧の最後で矢印キーを押すと先頭へ、先頭で押すと最後へ移動します。キーを押し続けた場合は端で止まります。
settings-gallery-show-hidden = 隠しファイルを表示
settings-gallery-show-hidden-help = 名前がドットで始まるファイルとフォルダも表示します。
settings-gallery-show-other-files = その他のファイルを表示
settings-gallery-show-other-files-help = { -app-name } で開けないファイルも汎用アイコンで表示します。
settings-gallery-show-archives = アーカイブを表示
settings-gallery-show-archives-help = コミックやその他のアーカイブ（zip、cbz、rar、7z など）もタイルとして表示します。アーカイブを直接開くことは引き続きできます。
settings-gallery-subfolder-depth = サブフォルダの階層
settings-gallery-subfolder-depth-help = この階層までのサブフォルダの画像も表示します（0: このフォルダのみ）。
settings-gallery-stacks = 関連ファイルをまとめる
settings-gallery-stacks-help = RAW+JPEG の組と連写を 1 つのタイルにまとめます。S キーで展開します。
settings-gallery-info-overlay = サムネイルに情報を表示
settings-gallery-info-overlay-help = 各サムネイルの上にサイズと露出を表示します。
settings-gallery-prefer-embedded = 埋め込みプレビューを優先
settings-gallery-prefer-embedded-help = 十分な大きさがあれば、ファイル内のプレビューを使います。ずっと速く、わずかに精度が下がります。
settings-gallery-sharper-raw-thumbnails = RAW のサムネイルを鮮明に
settings-gallery-sharper-raw-thumbnails-help = 編集した RAW 写真のサムネイルを 1024 px で現像します。より鮮明で、少し遅くなります。オフ: 512 px。
settings-gallery-rating-sidecars = 評価を XMP サイドカーに書き込む
settings-gallery-rating-sidecars-help = 評価、ラベル、フラグを画像の隣の .xmp ファイルにも保存し、他の写真アプリから読めるようにします。
settings-gallery-cache-size = サムネイルキャッシュの大きさ
settings-gallery-cache-size-help = キャッシュがいっぱいになると古いサムネイルから削除されます。
settings-gallery-cache-location = サムネイルキャッシュの場所
settings-gallery-cache-location-help = 高速なドライブ上のフォルダが最適です。
settings-gallery-clear-cache = サムネイルキャッシュを消去
settings-gallery-clear-cache-help = 保存されたサムネイルを削除します。閲覧すると再作成されます。

## Compare
# machine-drafted, needs native review

compare-side = 左右に並べる
compare-overlay = オーバーレイ
compare-difference = 差分
compare-flicker = フリッカー
compare-blend = ブレンド
compare-kind-absolute = 絶対誤差
compare-kind-squared = 二乗誤差
compare-kind-relative = 相対誤差
compare-scale-down = 感度を下げる
compare-scale-up = 感度を上げる
compare-scale = フルスケール { $value }
compare-space = リニア光、表示どおり
compare-space-help = 誤差と指標は、トーンマッピング後の表示どおりの画像をリニア光で、アルファを含むチャンネルごとに測定します。
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = { $width } × { $height } でサンプリング
compare-aligned = 左上揃え、{ $width } × { $height } を比較
compare-vs = { $b } と { $a } の比較
compare-overlay-label = { $b } を { $a } に重ねて { PERCENT($percent) }
compare-pending = 測定中…
# machine-drafted, needs native review
compare-diff-failed = 差分を表示できません: { $reason }
compare-no-overlap = 表示範囲に比較できるものがありません
compare-zoom-fit = 全体表示
compare-zoom = { PERCENT($percent) }
compare-showing = { $name } を表示中
compare-a11y-group = 比較
compare-a11y-blend = 画像間をブレンド
compare-a11y-zoom = ズーム
gallery-palette-compare-side = 左右に並べて比較
gallery-palette-compare-overlay = オーバーレイで比較
gallery-palette-compare-difference = 差分で比較: { $kind }
gallery-palette-compare-flicker = 比較中の画像をフリッカー表示
gallery-palette-compare-actual = { PERCENT(100) } で比較

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = Space または Ctrl+Enter で複数選択

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = カラープロファイル
gallery-facet-date = 撮影日
gallery-facet-picked = 採用
gallery-facet-rejected = 不採用
gallery-facet-flagged = 採用または不採用
gallery-facet-rated = レーティングあり
gallery-facet-labelled = ラベルあり

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = ビット深度
gallery-facet-pages = ページ数
gallery-facet-width = 幅
gallery-facet-height = 高さ
gallery-facet-megapixels = メガピクセル
gallery-picker-bits = { $bits } ビット
gallery-picker-pages = { $pages ->
   *[other] { $pages } ページ
}
gallery-picker-pixels = { $px } px
gallery-picker-megapixels = { $mp } MP
gallery-picker-group-any = グループ内のいずれか
gallery-picker-group-keep = 提案：残す
gallery-picker-group-discard = 提案：破棄
gallery-picker-ops = 比較
gallery-picker-op-eq = 等しい
gallery-picker-op-ge = 以上
gallery-picker-op-le = 以下
gallery-picker-ge = { $value } 以上
gallery-picker-gt = { $value } より大きい
gallery-picker-le = { $value } 以下
gallery-picker-lt = { $value } 未満

# Map view.
gallery-facet-place = 場所
gallery-palette-map = 地図
map-title = 地図
map-summary = 位置情報のある写真 { $count } 枚
map-photos = 写真 { $count } 枚
map-zoom-in = 拡大
map-zoom-out = 縮小
map-show-all = すべて表示
map-done = 完了
map-credit = 地図データ: Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = 地図: { $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = オンライン地図タイル
map-basemap-consent-server = 地図タイルは { $host } から取得します。
map-basemap-consent-sees = このサーバーには、お使いのインターネットアドレスと表示している範囲が伝わります。
map-basemap-consent-private = 写真、その名前、その位置は送信されません。
map-basemap-consent-cache = タイルはこのコンピュータに最大 { $size } まで保存されます。
map-basemap-allow = オンライン地図を使う
map-basemap-decline = オフライン地図のままにする
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = ベースマップを表示できません: { $reason }
map-basemap-bad-address = タイルサーバーのアドレスには {"{"}z{"}"}、{"{"}x{"}"}、{"{"}y{"}"} が必要です。
map-basemap-unknown-tiles = 地図ファイルのタイルの種類が不明です。
map-loading = 写真の位置を読み込み中…
map-empty = これらの写真には位置情報がありません
map-hidden = 写真の位置は非表示です (設定 › メタデータとプライバシー)
map-a11y-group = 写真の位置の地図
map-a11y-status = { $count } か所を表示、ズームレベル { $zoom }
