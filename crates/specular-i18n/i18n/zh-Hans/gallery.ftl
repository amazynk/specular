# machine-drafted, needs native review
# Specular — 图库（缩略图模式，DESIGN §10）与 设置 → 图库。简体中文。

## 命令面板与菜单

palette-gallery = 画廊
gallery-palette-view = 画廊视图…
gallery-palette-layout = 布局
gallery-palette-sort = 排序方式
gallery-palette-group = 分组方式
gallery-palette-filter = 筛选…
gallery-palette-quick-filters = 快速筛选
gallery-palette-details = 详细信息视图
gallery-palette-info-overlay = 在缩略图上显示信息
gallery-palette-stacks = 堆叠 RAW+JPEG 和连拍
gallery-palette-subfolders = 包含子文件夹
gallery-palette-compare = 比较所选图像
gallery-palette-rate = 评分…
gallery-palette-settings = 画廊设置…
gallery-view-menu = 视图
gallery-more-menu = 更多
gallery-open = 打开
gallery-new-window = 在新窗口中打开
gallery-reveal = 在文件夹中显示
gallery-properties = 属性
gallery-copy-paths = 复制路径
gallery-select-all = 全选
gallery-select-none = 取消全选
gallery-select-invert = 反向选择
gallery-next-unrated = 下一张未评分
gallery-stack-toggle = 展开或折叠堆叠
gallery-run-on = { $name }（{ $count } 个文件）
gallery-depth-off = 仅此文件夹
gallery-depth-all = 所有子文件夹
gallery-depth-levels = { $depth } 层子文件夹

## 状态与空白页

gallery-items = { $count } 项
gallery-selected = 已选择 { $count } / { $total } 项
gallery-shown-of = 显示 { $count } / { $total } 项
gallery-listing = 正在读取文件夹… { $count }
gallery-empty = 此文件夹中没有图像。
gallery-empty-filter = 没有符合筛选条件的图像。
gallery-list-error = 无法读取此文件夹：{ $reason }
gallery-no-folder = 未打开文件夹
gallery-kind-folder = 文件夹
gallery-drag-count = { $count } 个文件
gallery-badge-pages = { $count } 页

## 筛选栏

# machine-drafted, needs native review
# The filter field takes words (matched against names) and field:value terms; the field
# names and words of the query (rating, camera, ext, is:pick, …) stay in English.
gallery-filter-query-placeholder = 名称，或 rating>=3 camera:X-T5 …
gallery-filter-clear = 清除筛选
gallery-filter-rating = ★ { $stars } 星及以上
gallery-filter-label = 标签：{ $label }
gallery-filter-picks = 已选用
gallery-filter-hide-rejects = 隐藏已排除
gallery-filter-unrated = 未评分
gallery-filter-raw = RAW
gallery-filter-animated = 动画
gallery-filter-hdr = HDR
gallery-filter-landscape = 横向
gallery-filter-portrait = 纵向

## Query errors and facets

gallery-query-unknown-field = 未知字段“{ $field }”
gallery-query-unknown-field-suggest = 未知字段“{ $field }”：您是不是要找“{ $suggestion }”？
gallery-query-missing-value = “{ $field }”需要一个值
gallery-query-bad-number = “{ $value }”不是数字（{ $field }）
gallery-query-bad-date = “{ $value }”不是日期（{ $field }）：请使用 2024、2024-05 或 2024-05-12
gallery-query-bad-word = “{ $value }”不适用于 { $field }：请使用 { $words }
# machine-drafted, needs native review
gallery-query-bad-exif-tag = “{ $value }”不是 Exif 标签名
gallery-query-no-compare = { $field } 在此不能用 < 或 > 比较
gallery-query-unterminated = 缺少右引号
gallery-query-unavailable = “{ $field }”需要资料库（此处不可用）
gallery-facets = 分面
gallery-facets-counting = 正在统计…
gallery-facets-empty = 没有可统计的内容
gallery-facets-search = 搜索分面
gallery-facets-no-match = 没有匹配的值
# The gallery's top bar button (its tooltip): back to the start screen.
gallery-home = 开始屏幕
gallery-facet-format = 格式
gallery-facet-camera = 相机
gallery-facet-lens = 镜头
gallery-facet-rating = 评分
gallery-facet-year = 年份
gallery-facet-orientation = 方向
gallery-facet-label = 标签
gallery-facet-properties = 属性
gallery-facet-square = 方形
gallery-facet-alpha = 透明度
gallery-facet-gps = 位置
gallery-facet-multipage = 多页
# Screen readers: a facet value, e.g. “Camera: X-T5, 12 files”.
gallery-facet-a11y-value = { $facet }: { $value }, { $count ->
   *[other] { $count } 个文件
}

## 分组

# An event group's header: the first and the last day of a shooting session.
gallery-group-event = { $start } – { $end }
gallery-group-event-place = { $place } · { $dates }
gallery-group-this-folder = 此文件夹
gallery-group-unknown = 未知
gallery-group-unrated = 未评分

## 筛选照片

gallery-stars = { $stars } 星
gallery-rate-none = 无评分
gallery-pick = 选用
gallery-reject = 排除
gallery-unflag = 无旗标
gallery-label-red = 红色
gallery-label-yellow = 黄色
gallery-label-green = 绿色
gallery-label-blue = 蓝色
gallery-label-none = 无标签
gallery-compare-hint = 请选择 2 到 4 张图像进行比较。

## 文件

gallery-trash-count = 将 { $count } 个文件移到回收站
gallery-moved-to = 已将 { $count } 个文件移到 { $folder }
gallery-copied-to = 已将 { $count } 个文件复制到 { $folder }
gallery-move-failed = 无法移动 { $file }：{ $reason }
gallery-drop-move = 移动到 { $folder }
gallery-drop-copy = 复制到 { $folder }
gallery-move-here = 将所选项目移动到此处
gallery-copy-here = 将所选项目复制到此处
gallery-not-an-image = { $file } 不是 { -app-name } 可以打开的图像。
gallery-cache-cleared = 已清除缩略图缓存
# machine-drafted, needs native review
gallery-thumb-failed = 无缩略图：{ $reason }
gallery-cache-clear-failed = 无法清除缩略图缓存：{ $reason }

## 属性

gallery-prop-name = 名称
gallery-prop-folder = 文件夹
gallery-prop-size = 文件大小
gallery-prop-dimensions = 尺寸
gallery-prop-format = 格式
gallery-prop-date = 日期
gallery-prop-camera = 相机
gallery-prop-lens = 镜头
gallery-prop-rating = 评分
gallery-prop-label = 标签
gallery-prop-items = 项目

## 屏幕阅读器

gallery-a11y-list = { $folder }，{ $count } 项
gallery-a11y-dims = { $width } × { $height }
gallery-a11y-folder = 文件夹
gallery-a11y-up = 已返回 { $folder }
gallery-a11y-archive = 压缩包
gallery-a11y-stack = { $count } 张的堆叠
gallery-a11y-filter-bar = 筛选
gallery-a11y-filter-field = 按名称或字段筛选
gallery-a11y-chip = 筛选条件 { $term }
gallery-a11y-chip-remove = 按下即可移除
gallery-a11y-results = { $count ->
    [0] 没有匹配的项目
   *[other] 匹配 { $count } 个项目
}

## 设置 → 图库

settings-section-gallery = 画廊
settings-gallery-layout = 布局
settings-gallery-layout-help = “方格”将缩略图裁剪成网格；“对齐行”保留每张图像的形状；“不裁剪”将完整图像放入大小相同的格子。
settings-gallery-layout-squares = 方格
settings-gallery-layout-justified = 对齐行
settings-gallery-layout-uncropped = 不裁剪
settings-gallery-layout-details = 详细信息
settings-gallery-thumbnail-size = 缩略图大小
settings-gallery-thumbnail-size-help = 在画廊中也可以用 Ctrl+鼠标滚轮、双指缩放或 Ctrl+加号/减号调整。
settings-gallery-captions = 显示文件名
settings-gallery-captions-help = 在每张缩略图下方显示名称。
settings-gallery-badges = 显示标记
settings-gallery-badges-help = 表示 RAW、动画、HDR、多页、评分和标签的小标记。
settings-gallery-sort-order = 排序方式
settings-gallery-sort-order-help = 画廊中的顺序。文件夹始终排在最前。
settings-gallery-sort-order-name = 名称
settings-gallery-sort-order-date-taken = 拍摄日期
settings-gallery-sort-order-modified = 修改日期
settings-gallery-sort-order-size = 文件大小
settings-gallery-sort-order-type = 文件类型
settings-gallery-sort-order-dimensions = 尺寸
settings-gallery-sort-descending = 倒序
settings-gallery-sort-descending-help = 从最后到最前排序。
settings-gallery-group-by = 分组方式
settings-gallery-group-by-help = 滚动时停留在顶部的分区标题。
settings-gallery-group-by-none = 不分组
settings-gallery-group-by-day = 日
settings-gallery-group-by-month = 月
settings-gallery-group-by-year = 年
settings-gallery-group-by-event = 事件
settings-gallery-group-by-folder = 文件夹
settings-gallery-group-by-camera = 相机
settings-gallery-group-by-format = 格式
settings-gallery-group-by-rating = 评分
settings-gallery-group-by-target = 目标
settings-gallery-group-by-filter = 滤镜
settings-gallery-per-folder-view = 为每个文件夹记住视图
settings-gallery-per-folder-view-help = 布局、大小、排序和分组按文件夹分别保存；关闭后所有文件夹使用同一视图。
settings-gallery-open-folders-in-gallery = 在画廊中打开文件夹
settings-gallery-open-folders-in-gallery-help = 打开文件夹时，先显示缩略图而不是第一张图像。
settings-gallery-escape-opens-gallery = Esc 键打开画廊
settings-gallery-escape-opens-gallery-help = 没有可关闭的内容时，在查看器中按 Esc 键转到画廊。
settings-gallery-wrap-around = 方向键循环移动
settings-gallery-wrap-around-help = 到达列表末尾时，方向键会回到开头；在开头时则跳到末尾。按住按键不放时仍会停在末尾。
settings-gallery-show-hidden = 显示隐藏文件
settings-gallery-show-hidden-help = 包括名称以点开头的文件和文件夹。
settings-gallery-show-other-files = 显示其他文件
settings-gallery-show-other-files-help = 包括 { -app-name } 无法打开的文件，以通用图标显示。
settings-gallery-show-archives = 显示压缩包
settings-gallery-show-archives-help = 将漫画和其他压缩包（zip、cbz、rar、7z 等）显示为图块。直接打开压缩包仍然可用。
settings-gallery-subfolder-depth = 子文件夹层数
settings-gallery-subfolder-depth-help = 同时显示子文件夹中的图像，最多到此层数（0：仅此文件夹）。
settings-gallery-stacks = 堆叠相关文件
settings-gallery-stacks-help = 将 RAW+JPEG 对和连拍显示为一个图块。按 S 键展开堆叠。
settings-gallery-info-overlay = 在缩略图上显示信息
settings-gallery-info-overlay-help = 在每张缩略图上显示尺寸和曝光信息。
settings-gallery-prefer-embedded = 优先使用内嵌预览
settings-gallery-prefer-embedded-help = 文件中存储的预览足够大时就使用它。快得多，精度略低。
settings-gallery-sharper-raw-thumbnails = 更清晰的 RAW 缩略图
settings-gallery-sharper-raw-thumbnails-help = 编辑过的 RAW 照片以 1024 像素冲洗后生成缩略图。更清晰，稍慢一些。关闭时：512 像素。
settings-gallery-rating-sidecars = 将评分写入 XMP 附属文件
settings-gallery-rating-sidecars-help = 评分、标签和旗标还会写入图像旁的 .xmp 文件，其他照片应用可以读取。
settings-gallery-cache-size = 缩略图缓存大小
settings-gallery-cache-size-help = 缓存已满时会删除最旧的缩略图。
settings-gallery-cache-location = 缩略图缓存位置
settings-gallery-cache-location-help = 最好选择高速驱动器上的文件夹。
settings-gallery-clear-cache = 清除缩略图缓存
settings-gallery-clear-cache-help = 删除已保存的缩略图。浏览时会重新生成。

## Compare
# machine-drafted, needs native review

compare-side = 并排
compare-overlay = 叠加
compare-difference = 差异
compare-flicker = 闪烁
compare-blend = 混合
compare-kind-absolute = 绝对误差
compare-kind-squared = 平方误差
compare-kind-relative = 相对误差
compare-scale-down = 降低灵敏度
compare-scale-up = 提高灵敏度
compare-scale = 满量程 { $value }
compare-space = 线性光，按显示
compare-space-help = 误差和指标按图像显示时的线性光测量，在色调映射之后，按通道计算（包括 Alpha）。
compare-metrics = MAE { $mae }  ·  MSE { $mse }  ·  PSNR { $psnr } dB
compare-sampled = 按 { $width } × { $height } 采样
compare-aligned = 左上对齐，比较了 { $width } × { $height }
compare-vs = { $b } 与 { $a } 对比
compare-overlay-label = { $b } 叠加在 { $a } 上，{ PERCENT($percent) }
compare-pending = 正在测量…
# machine-drafted, needs native review
compare-diff-failed = 无法显示差异：{ $reason }
compare-no-overlap = 视图中没有可比较的内容
compare-zoom-fit = 适应窗口
compare-zoom = { PERCENT($percent) }
compare-showing = 正在显示 { $name }
compare-a11y-group = 比较
compare-a11y-blend = 在图像之间混合
compare-a11y-zoom = 缩放
gallery-palette-compare-side = 并排比较
gallery-palette-compare-overlay = 叠加比较
gallery-palette-compare-difference = 差异比较：{ $kind }
gallery-palette-compare-flicker = 在比较的图像之间闪烁
gallery-palette-compare-actual = 以 { PERCENT(100) } 比较

## Search value list (machine-drafted, needs native review)
gallery-picker-hint = 按空格键或 Ctrl+Enter 可多选

## Search value picker, part 2 (machine-drafted, needs native review)
gallery-facet-profile = 颜色描述文件
gallery-facet-date = 拍摄日期
gallery-facet-picked = 已选用
gallery-facet-rejected = 已排除
gallery-facet-flagged = 已选用或已排除
gallery-facet-rated = 已评分
gallery-facet-labelled = 有标签

## Search value picker, part 3 (machine-drafted, needs native review)

gallery-facet-bits = 位深度
gallery-facet-pages = 页数
gallery-facet-width = 宽度
gallery-facet-height = 高度
gallery-facet-megapixels = 百万像素
gallery-picker-bits = { $bits } 位
gallery-picker-pages = { $pages ->
   *[other] { $pages } 页
}
gallery-picker-pixels = { $px } 像素
gallery-picker-megapixels = { $mp } 百万像素
gallery-picker-group-any = 组中任意文件
gallery-picker-group-keep = 建议：保留
gallery-picker-group-discard = 建议：丢弃
gallery-picker-ops = 比较
gallery-picker-op-eq = 等于
gallery-picker-op-ge = 至少
gallery-picker-op-le = 至多
gallery-picker-ge = 至少 { $value }
gallery-picker-gt = 多于 { $value }
gallery-picker-le = 至多 { $value }
gallery-picker-lt = 少于 { $value }

# Map view.
gallery-facet-place = 地点
gallery-palette-map = 地图
map-title = 地图
map-summary = { $count } 张带位置的照片
map-photos = { $count } 张照片
map-zoom-in = 放大
map-zoom-out = 缩小
map-show-all = 全部显示
map-done = 完成
map-credit = 地图数据：Natural Earth
# machine-drafted, needs native review
# With a base map's tiles on screen: $credit is the tile source's credit ("© OpenStreetMap
# contributors").
map-credit-tiles = 地图：{ $credit } · Natural Earth
# The one-time explanation before Specular downloads map tiles from a server; one idea per
# line. $host is the server's name, $size the cache size ("256 MB").
map-basemap-consent-title = 在线地图图块
map-basemap-consent-server = 地图图块来自 { $host }。
map-basemap-consent-sees = 该服务器会看到你的网络地址和你查看的区域。
map-basemap-consent-private = 不会发送你的照片、照片名称及其位置。
map-basemap-consent-cache = 图块保存在这台电脑上，最多 { $size }。
map-basemap-allow = 使用在线地图
map-basemap-decline = 保留离线地图
# Why the base map is not shown; $reason is one of the lines below or a file error.
map-basemap-error = 未显示底图：{ $reason }
map-basemap-bad-address = 图块服务器地址需要包含 {"{"}z{"}"}、{"{"}x{"}"} 和 {"{"}y{"}"}。
map-basemap-unknown-tiles = 地图文件的图块类型未知。
map-loading = 正在读取照片位置…
map-empty = 这些照片都没有位置
map-hidden = 照片位置已隐藏（设置 › 元数据和隐私）
map-a11y-group = 照片位置地图
map-a11y-status = 显示 { $count } 个地点，缩放级别 { $zoom }
