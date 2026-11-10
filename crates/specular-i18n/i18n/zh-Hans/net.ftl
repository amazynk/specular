# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. 简体中文.

## The panel

# The panel's title.
net-title = 下载
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = 显示下载
# The panel when nothing is downloading any more.
net-empty = 没有正在进行的下载。
# Cancels one download.
net-stop = 停止

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = 来自 { $host } 的数据
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = 来自 { $host } 的图像图块
net-what-point-cloud = 来自 { $host } 的点云
net-what-map-tiles = 来自 { $host } 的地图图块
# A map service's capabilities, features or map images.
net-what-map-service = { $host } 上的地图服务
net-what-dicom = 来自 { $host } 的 DICOM 数据
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = 来自 { $host } 的 DICOM 序列 { $series }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = 向 { $host } 发送检查
net-what-omero = 来自 { $host } 的 OMERO 数据
net-what-zarr-chunks = 来自 { $host } 的 Zarr 数据块
net-what-zarr-metadata = 来自 { $host } 的 Zarr 元数据

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = 无法打开 { $address }：{ $reason }
# A Local AI model; $name is its name.
net-what-model = 模型 { $name }
net-what-update = 检查更新

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done }/{ $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done }/{ $total } { $count ->
   *[other] 个请求
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done }/{ $total } { $count ->
   *[other] 张图像
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } 张失败
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
   *[other] 个警告
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = 正在开始…
# Running, no time left known (many requests, or an unknown size).
net-running = 正在下载
net-done = 已完成
# Done, but $count of its requests failed.
net-done-failed = 已完成，{ $count } 个失败
# Cancelled with Stop.
net-stopped = 已停止
# $reason is one of the reasons below.
net-failed = 失败：{ $reason }
net-reason-offline = 无网络连接
net-reason-timeout = 服务器未及时响应
# $code is the HTTP status number.
net-reason-status = 服务器返回了 { $code }
net-reason-auth = 登录被拒绝
net-reason-other = 下载中断

## The indicator

# More downloads than shown in place.
net-more = 另有 { $count } 个
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = 下载：{ $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = 正在下载 { $what }
net-finished = 已下载 { $what }
net-failed-announce = 下载失败：{ $what }
net-stopped-announce = 下载已停止：{ $what }

## 无网络（设置 › 隐私）

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = 网络已在设置 › 隐私中关闭。
# The panel under the switch while the network is off.
privacy-off-title = 网络已关闭。
privacy-off-what = 更新检查、模型下载、DICOM、OMERO 和 Zarr 服务器、云端图像和地图均已关闭。
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = 由管理员在 { $file } 中设置。此开关无法重新打开网络。
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = 本次运行由 { $what } 设置。此开关无法重新打开网络。

## Open address… (File)

# machine-drafted, needs native review
address-title = 打开网址
# machine-drafted, needs native review
address-intro = Zarr 存储、云端 GeoTIFF (.tif)、COPC 点云 (.copc.laz)、STAC 条目或目录 (.json) 或 DICOMweb 服务器。
address-label = 网址（http、https 或 s3）
address-recent = 最近使用
address-open = 打开
address-network-off = 无法打开网址：网络已在设置 › 隐私中关闭。
address-problem-empty = 请输入或粘贴网址。
address-problem-scheme = 网址必须以 http://、https:// 或 s3:// 开头。
address-problem-host = 网址中没有服务器名称，或含有空格。
# machine-drafted, needs native review
address-problem-kind = { -app-name } 无法打开此网址。它可以打开 .zarr 存储、.tif、.copc.laz、STAC .json 或 DICOMweb 服务器。
