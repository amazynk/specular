# machine-drafted, needs native review
# Specular — Maps (the Maps profile): the map view of georeferenced and vector map files, its
# layer panel, attribute table, coordinates, measuring, elevation profile and map furniture.
# 简体中文.

keys-mode-map = 地图
# Cheat sheet groups of the Maps tab.
cheatsheet-map-layers = 图层
cheatsheet-map-coordinates = 坐标
cheatsheet-map-measure = 测量

## Palette entries and their one-line descriptions.
palette-map-layers = 显示或隐藏图层面板
palette-map-layers-help = 地图的图层：顺序、不透明度、可见性和样式。
palette-map-add-layer = 从文件添加图层…
palette-map-add-layer-help = 在此地图上叠加另一个地图文件，例如在 shapefile 上叠加 GPX 轨迹。
palette-map-zoom-to-layer = 缩放到图层
palette-map-zoom-to-layer-help = 完整显示图层面板中所选的图层。
palette-map-attribute-table = 显示或隐藏属性表
palette-map-attribute-table-help = 以表格显示所选图层的要素：排序、筛选、搜索和选择。
palette-map-coordinate-format = 下一种坐标格式
palette-map-coordinate-format-help = 十进制度、度分秒、UTM 或 MGRS。
palette-map-units = 公制或英制单位
palette-map-units-help = 长度和面积以米或英尺和英里表示。
palette-map-copy-coordinate = 复制坐标
palette-map-copy-coordinate-help = 复制指针下的位置，或地图中心的位置。
palette-map-go-to-coordinate = 转到坐标…
palette-map-go-to-coordinate-help = 以任意格式输入位置，地图将以此为中心。
palette-map-measure-distance = 测量距离
palette-map-measure-distance-help = 在地图上单击各点：地球表面上的长度和方位角。
palette-map-measure-area = 测量面积
palette-map-measure-area-help = 单击区域的各个角：地球表面上的面积和周长。
palette-map-elevation-profile = 高程剖面
palette-map-elevation-profile-help = 在高程模型上画一条线，以图表显示其高度。
palette-map-overview = 显示或隐藏鹰眼图
palette-map-overview-help = 整个范围的小地图，并标出屏幕上显示的部分。

## Why a map command is unavailable.
disabled-not-map = 仅适用于地图，且需开启地图工具
disabled-no-lonlat = 地图没有已知的坐标系
disabled-no-features = 地图没有要素
disabled-no-heights = 地图没有高度

## Settings › Maps.
settings-section-maps = 地图
settings-maps-coordinate-format = 坐标格式
settings-maps-coordinate-format-help = 位置的显示和复制方式。“转到坐标”可读取所有格式。
settings-maps-coordinate-format-dd = 十进制度
settings-maps-coordinate-format-dms = 度、分、秒
settings-maps-coordinate-format-utm = UTM
settings-maps-coordinate-format-mgrs = MGRS / USNG
settings-maps-units = 单位
settings-maps-units-help = 地图上的长度和面积：比例尺和测量。自动跟随“元数据 › 单位”。
settings-maps-units-auto = 自动
settings-maps-units-metric = 公制
settings-maps-units-imperial = 英制

## The pointer readout. $position is written in the chosen format.
map-position = { $format ->
    [utm] UTM { $position }
    [mgrs] MGRS { $position }
    [dms] { $position }
   *[dd] 纬度, 经度 { $position }
}
map-height = 高度 { $height }
map-height-label = 高度
# A length or area with its unit symbol ("12.3 km", "4.5 ha").
map-length = { $value } { $unit }
map-scale-label = 比例尺：{ $length }
map-north-label = 指北针：北方位于从上方顺时针 { $degrees }°
# The coordinate formats, for the toast after "Next coordinate format".
map-format-name = { $format ->
    [dms] 度分秒
    [utm] UTM
    [mgrs] MGRS
   *[dd] 十进制度
}
map-format-toast = 坐标格式：{ $format }
map-units-metric = 单位：米和千米
map-units-imperial = 单位：英尺和英里
toast-copied-coordinate = 已复制坐标

## Go to coordinate.
map-goto-hint = 十进制度、DMS、UTM 或 MGRS。按 Enter 转到。
map-goto-empty = 请输入位置。
map-goto-unrecognised = 不是已知格式的位置。
map-goto-range = 纬度或经度超出范围。
map-goto-no-way = 此地图无法定位位置。
map-goto-outside = 该位置在地图之外。
map-goto-toast = 已以 { $position } 为中心

## The layer panel.
map-layers-title = 图层：显示 { $shown } / { $total }
# Under a layer's name: its kind, features and opacity.
map-layer-sub = { $kind } · { $features } · { PERCENT($opacity) }
# A layer's row for screen readers.
map-layer-row = { $name }：{ $kind }，{ $features ->
   *[other] { $features } 个要素
}，不透明度 { PERCENT($opacity) }
map-kind-area = 面
map-kind-line = 线
map-kind-point = 点
map-kind-empty = 无要素
map-layer-visible = 显示或隐藏图层
map-layer-opacity-down = 降低不透明度
map-layer-opacity-up = 提高不透明度
map-layer-colour = 下一种颜色
map-layer-width = 下一种线宽
map-layer-point = 下一种点大小
map-layer-up = 上移
map-layer-down = 下移
map-layer-remove = 移除
map-layer-keep = 保留
map-layer-remove-ask = 要从地图中移除 { $name } 吗？文件仍保留在磁盘上。
map-layer-adding = 正在添加图层…
map-removed = 已从地图中移除 { $name }
map-added = { $layers ->
   *[other] 已添加 { $name }：{ $layers } 个图层
}
map-add-failed = 无法添加 { $name }：{ $reason }
map-add-filter = 地图文件
map-add-not-map = 它不是地图。
map-add-not-vector = 它没有要素。
map-add-no-crs = 它没有投影文件 (.prj)。

## Identify.
map-identify-more = 属性表中还有 { $count } 个
map-cell = 像元 { $x }, { $y }
map-band = 波段 { $n }
map-no-data = 无数据

## The attribute table.
map-table-title = { $layer }：{ $shown } / { $total } 个要素，已选择 { $selected } 个
map-table-search = 搜索，或 字段 = 值
map-table-selected-only = 仅显示所选
map-table-empty = 没有匹配的要素。
map-table-many = 大量
map-table-stats-number = { $field }：{ $count } 个值，{ $empty } 个为空 · 最小 { $min } · 最大 { $max } · 平均 { $mean } · 总和 { $sum } · 标准差 { $sd }
map-table-stats-text = { $field }：{ $count } 个值，{ $empty } 个为空 · { $distinct } 个不同值
map-table-stats-text-top = { $field }：{ $count } 个值，{ $empty } 个为空 · { $distinct } 个不同值 · 最常见 { $top }（{ $times }×）

## Measuring and the elevation profile.
map-measure-hint-distance = 沿路线单击各点。
map-measure-hint-area = 单击区域的各个角。
map-measure-hint-profile = 沿线单击各点。
map-measure-keys = Enter 完成。Backspace 删除最后一个点。Esc 关闭。
map-measure-distance = 距离 { $length }
map-measure-bearing = 方位角 { $bearing }
map-measure-segment = 最后一段 { $length }，方位角 { $bearing }
map-measure-area = 面积 { $area }
map-measure-perimeter = 周长 { $length }
map-bearing-value = { $degrees }°
map-profile-summary = 长度 { $length } · 最低 { $min } · 最高 { $max } · 爬升 { $climb } · 下降 { $descent }
map-profile-no-heights = 长度 { $length }。沿线没有高度。
map-profile-at = { $distance }：{ $height }
map-profile-at-none = { $distance }：无高度
