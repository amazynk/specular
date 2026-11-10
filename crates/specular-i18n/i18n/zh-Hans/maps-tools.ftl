# machine-drafted, needs native review
# Specular — Maps tools: terrain (slope, aspect, contours, 3-D), time, wind and weather colours,
# online map services, photos and geotagging along GPS tracks, and map export.
# 简体中文.

## Cheat sheet groups of the Maps tab.
cheatsheet-map-terrain = 地形
cheatsheet-map-time = 时间和天气
cheatsheet-map-online = 在线服务
cheatsheet-map-photos = 照片和 GPS
cheatsheet-map-output = 输出

## Palette entries and their one-line descriptions.
palette-map-slope = 坡度着色
palette-map-slope-help = 按陡峭程度为高程模型着色。
palette-map-aspect = 坡向着色
palette-map-aspect-help = 按坡面朝向为高程模型着色。
palette-map-contours = 显示或隐藏等高线
palette-map-contours-help = 高程模型上的等高线，每隔几条标注一次。
palette-map-terrain-3d = 三维地形
palette-map-terrain-3d-help = 将高程模型显示为三维表面，并覆盖其颜色。
palette-map-time = 显示或隐藏时间滑块
palette-map-time-help = 逐步浏览网格的各个时间，或沿轨迹的时间浏览。
palette-map-play = 播放或暂停时间
palette-map-play-help = 让时间滑块自动逐步运行。
palette-map-wind = 显示或隐藏风向箭头
palette-map-wind-help = 由网格的 u 和 v 分量得出的风速和风向箭头。
palette-map-weather-colours = 下一种天气色图
palette-map-weather-colours-help = 适用于气温、降水、风和气压的色图。
palette-map-services = 在线地图服务…
palette-map-services-help = 从 WMS、WMTS、WFS、OGC API 或 STAC 服务器添加图层。
palette-map-photos-on-track = 显示或隐藏轨迹上的照片
palette-map-photos-on-track-help = 按拍摄时间将文件夹中的照片放到 GPS 轨迹上。
palette-map-geotag = 根据轨迹为照片添加地理标记…
palette-map-geotag-help = 将轨迹的位置写入文件夹中照片的 XMP 附属文件。
palette-map-export = 导出地图…
palette-map-export-help = 按比例尺将地图保存为 PNG 或 PDF，带图例、比例尺和指北针。
palette-map-print = 按比例尺打印地图…
palette-map-print-help = 按所选比例尺打印地图，例如 1:{ NUMBER(25000) }。
palette-map-copy-extent = 复制范围
palette-map-copy-extent-help = 将屏幕上的区域复制为边界框或 WKT。

## Why a map command is unavailable.
disabled-no-time = 地图没有时间
disabled-no-wind = 地图没有风分量（u 和 v）
disabled-no-track = 没有带时间的 GPS 轨迹
disabled-not-grid = 地图不是数值网格

## Settings › Maps and Metadata.
settings-maps-services = 在线地图服务
settings-maps-services-help = 从 WMS、WMTS、WFS、OGC API 和 STAC 服务器添加图层。关闭：不获取任何内容。
settings-maps-services-allowed = 已允许的地图服务器
settings-maps-services-allowed-help = 你在了解它们能看到什么之后同意使用的服务器。
settings-metadata-basemap-service = 地图服务图层
settings-metadata-basemap-service-help = 在地图的在线服务面板中选择的图层。

## Terrain.
# A compass point, short: n, ne, e, se, s, sw, w, nw.
map-compass = { $point ->
    [ne] 东北
    [e] 东
    [se] 东南
    [s] 南
    [sw] 西南
    [w] 西
    [nw] 西北
   *[n] 北
}
map-slope-aspect = 坡度 { $slope }° · 朝向{ $direction }（{ $aspect }°）
map-slope-flat = 坡度 { $slope }° · 平坦
map-slope-on = 坡度着色
map-aspect-on = 坡向着色
map-relief-off = 高度着色
map-contours-on = 已显示等高线
map-contours-off = 已隐藏等高线
map-legend-slope = 坡度
map-legend-aspect = 坡向：坡面朝向
map-legend-degrees = { $degrees }°
map-legend-degrees-more = { $degrees }° 及以上
map-terrain-3d-on = 三维地形。按 Esc 返回地图。
map-terrain-3d-on-exaggerated = 三维地形，高度 × { $times }。按 Esc 返回地图。
map-terrain-exaggeration = 高度夸大 { $times } 倍。
map-terrain-drape = 地图颜色

## Time, weather and wind.
map-time-grid = { $when } · 第 { $step } / { $steps } 步
map-time-track = { $when } · 已过 { $elapsed }
map-time-speed = × { $times }
map-time-play = 播放
map-time-pause = 暂停
map-weather-map = 颜色：{ $name }
map-weather-file = 使用文件中的颜色
map-wind-on = 已显示风向箭头
map-wind-off = 已隐藏风向箭头
map-wind-at = 风 { $speed } { $unit }，来自 { $degrees }°（{ $direction }）

## Copy extent.
map-extent-copied = 已复制边界框。再次复制可得到 WKT。
map-extent-copied-wkt = 已将范围复制为 WKT

## Online map services.
map-services-title = 在线地图服务
map-service-off = 在线地图服务已关闭。
map-service-off-why = 开启后可从互联网上的服务器添加图层。
map-service-turn-on = 开启
map-service-close = 关闭
map-service-back = 返回
map-service-cancel = 取消
map-service-connect = 连接
map-service-allow = 允许
map-service-remove = 移除
map-service-download = 下载
map-service-list-hint = Enter 打开服务器。Delete 移除它。
map-service-add = 添加服务器…
map-service-add-sub = WMS、WMTS、WFS、OGC API 或 STAC
map-service-add-hint = 服务器的地址。按 Enter 连接。
map-service-bad-address = 不是网址（需 http 或 https，不含用户名）。
map-service-server-sub = { $kind } · { $host }
map-service-consent-title = 在线地图服务
map-service-consent-server = Specular 将连接 { $host }。
map-service-consent-sees = 该服务器能看到你的互联网地址和你请求的地图区域。
map-service-consent-private = 不会发送你的文件及其名称。
map-service-asking = 正在询问 { $host }…
map-service-downloading = 正在从 { $host } 下载…
map-service-layers-hint = { $kind } 图层。按 Enter 使用其中一个。
map-service-no-layers = 服务器未列出任何图层。
map-service-offer-tiles = 图块：成为照片地图的底图
map-service-offer-features = 要素：按屏幕上的区域添加到此地图
map-service-offer-search = 图像：搜索屏幕上的区域
map-service-refused-mercator = 未提供底图所需的 Web Mercator (EPSG:3857)。
map-service-refused-format = 未提供 PNG 或 JPEG 图像。
map-service-refused-geojson = 未提供 GeoJSON，即此处读取的格式。
map-service-items-hint = 找到 { $count } 张图像。按 Enter 下载其中一张。
map-service-no-items = 此处未找到图像。
map-service-item-cloud = { $when } · 云量 { PERCENT($cloud) }
map-service-item-no-cog = 没有 Cloud-Optimized GeoTIFF
map-service-download-ask = 要从 { $host } 下载 { $size } 吗？
map-service-download-ask-unknown = 要从 { $host } 下载此图像吗？其大小未知。
map-service-download-cache = 它会保存在地图缓存中并打开。
map-service-size-mb = { $size } MB
map-service-size-gb = { $size } GB
map-service-too-big = 该图像为 { $size }，太大，无法下载。
map-service-opened = 正在打开已下载的图像
map-service-basemap = { $layer } 现在是照片地图的底图。照片地图在首次下载前会先询问。
map-service-status = 服务器返回了 { $status }。
map-service-unknown = 此版本无法读取这种地图服务。
map-service-exception = 服务器提示：{ $message }
map-service-malformed = 服务器的应答已损坏：{ $reason }
map-service-remove-ask = 要从已保存的服务器中移除“{ $server }”吗？
map-service-remove-detail = 磁盘上的内容不会丢失。地图上已有的图层会保留到地图关闭为止。
map-service-removed = 已移除 { $server }
map-service-saved = 已保存的地图服务器
map-service-saved-none = 暂无。可在地图的在线服务面板中添加。
map-service-keywords = WMS WMTS WFS OGC API STAC 服务器

## Photos and geotagging.
map-geotag-title = 根据轨迹为照片添加地理标记
map-photos-on = 轨迹上的照片
map-photos-off = 已隐藏轨迹上的照片
map-geotag-offset = 相机时钟：与 GPS 时间相差 { $offset }。← → 每次调整一分钟，按住 Shift 调整一小时，按住 Alt 调整一秒。
map-geotag-reading = 正在读取照片…
map-geotag-summary = { $total } 张照片中有 { $count } 张获得位置。
map-geotag-sidecars = 位置写入照片旁的 XMP 附属文件。照片文件不会更改。
map-geotag-writing = 正在写入…
map-geotag-write = 写入附属文件…
map-geotag-guess = 估算偏移
map-geotag-will-write = 获得 { $position }
map-geotag-has-position = 已有位置：保持不变
map-geotag-no-time = 没有拍摄时间：保持不变
map-geotag-outside = 拍摄时间在轨迹外 { $offset }：保持不变
map-geotag-gap = 位于轨迹的空缺处（距定位点 { $seconds } 秒）：保持不变
map-geotag-nothing = 在此偏移下没有照片获得位置。
map-geotag-confirm = 要将位置写入 { $count ->
   *[other] { $count } 个 XMP 附属文件
}吗？照片本身不会更改。
map-geotag-confirm-ok = 写入附属文件
map-geotag-done = { $count ->
   *[other] 已写入 { $count } 个附属文件
}
map-geotag-done-failed = 已写入 { $count } 个附属文件；{ $failed } 个失败：{ $reason }
map-geotag-not-sidecar = 不是附属文件名。

## Export and printing.
map-export-title = 导出地图
map-print-title = 按比例尺打印地图
map-export-keys = ↑ ↓ 选择，← → 更改，Enter 执行。
map-export-format = 格式
map-export-scale = 比例尺
map-export-scale-value = 1:{ $n }
map-export-resolution = 分辨率
map-export-dpi = { $dpi } dpi
map-export-paper = 纸张
map-export-size = { $width } × { $height } px · 地面 { $across } × { $down } km
map-export-save = 保存…
map-export-print = 打印…
map-export-too-large = 此比例尺下太大：{ $width } × { $height } px。请选择更小的比例尺或分辨率。
map-export-no-scale = 此地图没有地面比例尺。
map-export-no-gpu = 导出需要 GPU。
map-export-subtitle = 比例尺 { $scale } · { $crs }
map-export-data = 数据：{ $sources }
map-export-saved = 已保存 { $name }
map-export-printing = 正在打印…
map-export-printed = 已发送到打印机
map-export-print-failed = 未打印：{ $reason }
