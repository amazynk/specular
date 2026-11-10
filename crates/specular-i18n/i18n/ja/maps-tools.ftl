# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.
# 日本語.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = 地形
cheatsheet-map-time = 時間と気象
cheatsheet-map-online = オンラインサービス
cheatsheet-map-photos = 写真と GPS
cheatsheet-map-output = 出力

## Palette entries and their one-line descriptions.
palette-map-slope = 傾斜の色分け
palette-map-slope-help = 標高モデルを傾斜の急さで色分けします。
palette-map-aspect = 斜面方位の色分け
palette-map-aspect-help = 標高モデルを斜面の向きで色分けします。
palette-map-contours = 等高線の表示/非表示
palette-map-contours-help = 標高モデルに重ねる等高線。数本ごとにラベルを付けます。
palette-map-terrain-3d = 3D 地形
palette-map-terrain-3d-help = 標高モデルを、色を重ねた 3D の面として表示します。
palette-map-time = タイムスライダーの表示/非表示
palette-map-time-help = グリッドの時刻、またはトラックの時刻を順に進みます。
palette-map-play = 時間の再生/一時停止
palette-map-play-help = タイムスライダーを自動で 1 ステップずつ進めます。
palette-map-wind = 風の矢印の表示/非表示
palette-map-wind-help = グリッドの u、v 成分から求めた風速と風向の矢印。
palette-map-weather-colours = 次の気象カラーマップ
palette-map-weather-colours-help = 気温、降水、風、気圧向けのカラーマップ。
palette-map-services = オンライン地図サービス…
palette-map-services-help = WMS、WMTS、WFS、OGC API、STAC サーバーからレイヤーを追加します。
palette-map-photos-on-track = トラック上の写真の表示/非表示
palette-map-photos-on-track-help = フォルダーの写真を撮影時刻に従って GPS トラック上に配置します。
palette-map-geotag = トラックから写真にジオタグ付け…
palette-map-geotag-help = トラックの位置を、フォルダーの写真の XMP サイドカーに書き込みます。
palette-map-export = 地図を書き出し…
palette-map-export-help = 地図を縮尺を指定して PNG または PDF で保存します。凡例、縮尺バー、方位記号付き。
palette-map-print = 縮尺を指定して地図を印刷…
palette-map-print-help = 1:{ NUMBER(25000) } などの選んだ縮尺で地図を印刷します。
palette-map-copy-extent = 範囲をコピー
palette-map-copy-extent-help = 画面上の範囲を境界ボックス、または WKT としてコピーします。

## Why a map command is unavailable.
disabled-no-time = 地図に時刻がありません
disabled-no-wind = 地図に風の成分 (u と v) がありません
disabled-no-track = 時刻付きの GPS トラックがありません
disabled-not-grid = 地図が値のグリッドではありません

## Settings › Maps and Metadata.
settings-maps-services = オンライン地図サービス
settings-maps-services-help = WMS、WMTS、WFS、OGC API、STAC サーバーからレイヤーを追加します。オフ：何も取得しません。
settings-maps-services-allowed = 許可した地図サーバー
settings-maps-services-allowed-help = 何が見えるかの説明を読んだうえで同意したサーバー。
settings-metadata-basemap-service = 地図サービスのレイヤー
settings-metadata-basemap-service-help = 地図のオンラインサービスパネルで選んだレイヤー。

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] 北東
    [e] 東
    [se] 南東
    [s] 南
    [sw] 南西
    [w] 西
    [nw] 北西
   *[n] 北
}
map-slope-aspect = 傾斜 { $slope }° · { $direction } 向き ({ $aspect }°)
map-slope-flat = 傾斜 { $slope }° · 平坦
map-slope-on = 傾斜の色分け
map-aspect-on = 斜面方位の色分け
map-relief-off = 高さの色分け
map-contours-on = 等高線オン
map-contours-off = 等高線オフ
map-legend-slope = 傾斜
map-legend-aspect = 斜面方位：斜面の向き
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° 以上
map-terrain-3d-on = 3D 地形。Esc で地図に戻ります。
map-terrain-3d-on-exaggerated = 3D 地形、高さ × { $times }。Esc で地図に戻ります。
map-terrain-exaggeration = 高さを { $times } 倍に強調しました。
map-terrain-drape = 地図の色

## Time, weather and wind.
map-time-grid = { $when } · { $steps } 中 { $step }
map-time-track = { $when } · 開始から { $elapsed }
map-time-speed = × { $times }
map-time-play = 再生
map-time-pause = 一時停止
map-weather-map = 色：{ $name }
map-weather-file = ファイルの色
map-wind-on = 風の矢印オン
map-wind-off = 風の矢印オフ
map-wind-at = 風 { $speed } { $unit }、{ $degrees }° ({ $direction }) から

## Copy extent.
map-extent-copied = 境界ボックスをコピーしました。もう一度コピーすると WKT になります。
map-extent-copied-wkt = 範囲を WKT としてコピーしました

## Online map services.
map-services-title = オンライン地図サービス
map-service-off = オンライン地図サービスはオフです。
map-service-off-why = オンにすると、インターネット上のサーバーからレイヤーを追加できます。
map-service-turn-on = オンにする
map-service-close = 閉じる
map-service-back = 戻る
map-service-cancel = キャンセル
map-service-connect = 接続
map-service-allow = 許可
map-service-remove = 削除
map-service-download = ダウンロード
map-service-list-hint = Enter でサーバーを開き、Delete で削除します。
map-service-add = サーバーを追加…
map-service-add-sub = WMS、WMTS、WFS、OGC API、STAC
map-service-add-hint = サーバーのアドレス。Enter で接続します。
map-service-bad-address = Web アドレスではありません (http または https、ユーザー名なし)。
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = オンライン地図サービス
map-service-consent-server = Specular は { $host } に接続します。
map-service-consent-sees = そのサーバーには、あなたのインターネットアドレスと要求した地図の範囲が見えます。
map-service-consent-private = ファイルとその名前は送信されません。
map-service-asking = { $host } に問い合わせ中…
map-service-downloading = { $host } からダウンロード中…
map-service-layers-hint = { $kind } レイヤー。Enter で使用します。
map-service-no-layers = サーバーにレイヤーがありません。
map-service-offer-tiles = タイル：写真マップのベースマップになります
map-service-offer-features = 地物：画面上の範囲についてこの地図に追加します
map-service-offer-search = 画像：画面上の範囲を検索します
map-service-refused-mercator = ベースマップに必要な Web メルカトル (EPSG:3857) で提供されていません。
map-service-refused-format = PNG または JPEG 画像で提供されていません。
map-service-refused-geojson = ここで読める形式の GeoJSON で提供されていません。
map-service-items-hint = 画像が { $count } 件見つかりました。Enter でダウンロードします。
map-service-no-items = ここには画像がありません。
map-service-item-cloud = { $when } · 雲量 { PERCENT($cloud) }
map-service-item-no-cog = Cloud-Optimized GeoTIFF なし
map-service-download-ask = { $host } から { $size } をダウンロードしますか？
map-service-download-ask-unknown = { $host } からこの画像をダウンロードしますか？サイズは不明です。
map-service-download-cache = 地図キャッシュに保存して開きます。
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = 画像は { $size } で、ダウンロードするには大きすぎます。
map-service-opened = ダウンロードした画像を開いています
map-service-basemap = { $layer } が写真マップのベースマップになりました。写真マップは最初のダウンロードの前に確認します。
map-service-status = サーバーの応答：{ $status }。
map-service-unknown = このバージョンで読める地図サービスではありません。
map-service-exception = サーバーのメッセージ：{ $message }
map-service-malformed = サーバーの応答が壊れています：{ $reason }
map-service-remove-ask = 「{ $server }」を保存済みサーバーから削除しますか？
map-service-remove-detail = ディスク上のものは失われません。地図に追加済みのレイヤーは地図を閉じるまで残ります。
map-service-removed = { $server } を削除しました
map-service-saved = 保存済みの地図サーバー
map-service-saved-none = まだありません。地図のオンラインサービスパネルで追加してください。
map-service-keywords = WMS WMTS WFS OGC API STAC サーバー 地図サービス

## Photos and geotagging.
map-geotag-title = トラックから写真にジオタグ付け
map-photos-on = トラック上の写真
map-photos-off = トラック上の写真を非表示
map-geotag-offset = カメラの時計：GPS 時刻との差 { $offset }。← → で 1 分ずつ、Shift で 1 時間、Alt で 1 秒ずつ変更します。
map-geotag-reading = 写真を読み込み中…
map-geotag-summary = { $total } 枚中 { $count } 枚の写真に位置が付きます。
map-geotag-sidecars = 位置は写真の隣の XMP サイドカーに書き込まれます。写真ファイルは変更されません。
map-geotag-writing = 書き込み中…
map-geotag-write = サイドカーを書き込む…
map-geotag-guess = 時差を推定
map-geotag-will-write = { $position } を付与
map-geotag-has-position = すでに位置あり：そのまま
map-geotag-no-time = 撮影時刻なし：そのまま
map-geotag-outside = トラックの範囲外 { $offset } で撮影：そのまま
map-geotag-gap = トラックの途切れ (測位点から { $seconds } 秒)：そのまま
map-geotag-nothing = この時差では位置が付く写真はありません。
map-geotag-confirm = { $count ->
   *[other] { $count } 個の XMP サイドカー
}に位置を書き込みますか？写真自体は変更されません。
map-geotag-confirm-ok = サイドカーを書き込む
map-geotag-done = { $count ->
   *[other] サイドカーを { $count } 個書き込みました
}
map-geotag-done-failed = サイドカーを { $count } 個書き込みました。{ $failed } 個失敗：{ $reason }
map-geotag-not-sidecar = サイドカーのファイル名ではありません。

## Export and printing.
map-export-title = 地図を書き出し
map-print-title = 縮尺を指定して地図を印刷
map-export-keys = ↑ ↓ で選択、← → で変更、Enter で実行。
map-export-format = 形式
map-export-scale = 縮尺
map-export-scale-value = 1:{ $n }
map-export-resolution = 解像度
map-export-dpi = { $dpi } dpi
map-export-paper = 用紙
map-export-size = { $width } × { $height } px · 地上 { $across } × { $down } km
map-export-save = 保存…
map-export-print = 印刷…
map-export-too-large = この縮尺では大きすぎます：{ $width } × { $height } px。小さい縮尺か解像度を選んでください。
map-export-no-scale = この地図には地上の縮尺がありません。
map-export-no-gpu = 書き出しには GPU が必要です。
map-export-subtitle = 縮尺 { $scale } · { $crs }
map-export-data = データ：{ $sources }
map-export-saved = { $name } を保存しました
map-export-printing = 印刷中…
map-export-printed = プリンターに送信しました
map-export-print-failed = 印刷できませんでした：{ $reason }
