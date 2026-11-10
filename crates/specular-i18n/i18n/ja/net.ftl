# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. 日本語.

## The panel

# The panel's title.
net-title = ダウンロード
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = ダウンロードを表示
# The panel when nothing is downloading any more.
net-empty = ダウンロード中のものはありません。
# Cancels one download.
net-stop = 停止

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = { $host } のデータ
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = { $host } の画像タイル
net-what-point-cloud = { $host } の点群
net-what-map-tiles = { $host } の地図タイル
# A map service's capabilities, features or map images.
net-what-map-service = { $host } の地図サービス
net-what-dicom = { $host } の DICOM データ
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = { $host } の DICOM シリーズ { $series }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = { $host } への検査の送信
net-what-omero = { $host } の OMERO データ
net-what-zarr-chunks = { $host } の Zarr チャンク
net-what-zarr-metadata = { $host } の Zarr メタデータ

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = { $address } を開けません：{ $reason }
# A Local AI model; $name is its name.
net-what-model = モデル { $name }
net-what-update = アップデートの確認

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } / { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } / { $total } { $count ->
   *[other] 件のリクエスト
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } / { $total } { $count ->
   *[other] 枚の画像
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } 件失敗
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
   *[other] 件の警告
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = 開始中…
# Running, no time left known (many requests, or an unknown size).
net-running = ダウンロード中
net-done = 完了
# Done, but $count of its requests failed.
net-done-failed = 完了、{ $count } 件失敗
# Cancelled with Stop.
net-stopped = 停止しました
# $reason is one of the reasons below.
net-failed = 失敗しました: { $reason }
net-reason-offline = 接続がありません
net-reason-timeout = サーバーが時間内に応答しませんでした
# $code is the HTTP status number.
net-reason-status = サーバーの応答: { $code }
net-reason-auth = サインインが拒否されました
net-reason-other = ダウンロードが途切れました

## The indicator

# More downloads than shown in place.
net-more = ほか { $count } 件
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = ダウンロード: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = { $what } をダウンロード中
net-finished = { $what } をダウンロードしました
net-failed-announce = ダウンロードに失敗しました: { $what }
net-stopped-announce = ダウンロードを停止しました: { $what }

## ネットワークなし（設定 › プライバシー）

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = 設定 › プライバシーでネットワークがオフになっています。
# The panel under the switch while the network is off.
privacy-off-title = ネットワークはオフです。
privacy-off-what = アップデートの確認、モデルのダウンロード、DICOM・OMERO・Zarr サーバー、クラウド画像、地図はオフです。
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = 管理者が { $file } で設定しています。このスイッチではネットワークをオンに戻せません。
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = 今回の実行では { $what } で設定されています。このスイッチではネットワークをオンに戻せません。

## Open address… (File)

# machine-drafted, needs native review
address-title = アドレスを開く
# machine-drafted, needs native review
address-intro = Zarr ストア、クラウド上の GeoTIFF (.tif)、COPC 点群 (.copc.laz)、STAC アイテムまたはカタログ (.json)、または DICOMweb サーバー。
address-label = アドレス (http、https または s3)
address-recent = 最近使ったもの
address-open = 開く
address-network-off = アドレスを開けません: 設定 › プライバシーでネットワークがオフになっています。
address-problem-empty = アドレスを入力するか貼り付けてください。
address-problem-scheme = アドレスは http://、https:// または s3:// で始まる必要があります。
address-problem-host = アドレスにサーバー名がないか、スペースが含まれています。
# machine-drafted, needs native review
address-problem-kind = { -app-name } で開けるアドレスではありません。開けるのは .zarr ストア、.tif、.copc.laz、STAC の .json、DICOMweb サーバーです。
