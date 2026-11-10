# machine-drafted, needs native review
# Specular — 设置窗口。简体中文。

## 窗口界面

settings-title = 设置
settings-search-placeholder = 搜索设置
settings-search-no-results = 没有与 { $query } 匹配的设置
settings-reset = 重置
settings-reset-section = 重置此部分
settings-modified = 已修改
settings-default-value = 默认：{ $value }
settings-restart-required = 重新启动后生效
settings-restart-now = 立即重新启动
settings-undo = 撤销
settings-redo = 重做
settings-value-on = 开
settings-value-off = 关
settings-value-auto = 自动
settings-value-system = 跟随系统
settings-value-system-language = 系统默认
settings-value-follow-language = 跟随界面语言
settings-value-default-location = 默认位置
settings-choose = 选择…
settings-clear = 清除
settings-file-reloaded = settings.toml 已在 { -app-name } 之外被更改，现已重新载入。
settings-warning-unknown-key = settings.toml 中有未知设置 { $key }。它会保留在文件中，但会被忽略。
settings-warning-invalid-value = { $key } 的值无效：{ $reason } 将改用默认值。
settings-warning-newer-version = settings.toml 由更新版本的 { -app-name } 写入。此版本无法识别的设置将被保留。
settings-warning-syntax = 无法读取 settings.toml（第 { $line } 行）：{ $reason } 在修复之前将使用默认值。

## 验证错误（也由 `specular --set` 输出）

settings-error-unknown-key = 没有名为 { $key } 的设置。
settings-error-unknown-key-suggestion = 没有名为 { $key } 的设置。你是不是要找 { $suggestion }？
settings-error-not-bool = 应为 on 或 off（true 或 false）。
settings-error-not-integer = 应为整数。
settings-error-not-number = 应为数字。
settings-error-out-of-range = 必须介于 { $min } 和 { $max } 之间。
settings-error-out-of-range-auto = 必须为 0（自动）或介于 { $min } 和 { $max } 之间。
settings-error-unknown-option = 应为以下之一：{ $options }。
settings-error-bad-colour = 应为颜色值，例如 #1E1E1E 或 #1E1E1E80。
settings-error-bad-language = 应为语言标记，例如 de、pt-BR 或 zh-Hans，或者“system”。
settings-error-bad-path = 应为文件或文件夹路径。
settings-error-not-settable = 此条目是命令，没有值。
settings-error-syntax = 应为 key=value 格式，例如 viewing.wheel_behaviour=navigate。
# machine-drafted, needs native review
settings-error-bad-text = 不能包含制表符、换行符或其他控制字符。
settings-error-invalid = { $reason }
settings-error-tile-url = 需要包含 {"{"}z{"}"}、{"{"}x{"}"} 和 {"{"}y{"}"} 的 http 或 https 地址。

## 单位

unit-percent = { PERCENT($value) }
unit-milliseconds = { $value } 毫秒
unit-seconds = { $value } 秒
unit-mebibytes = { $value } MiB
# machine-drafted, needs native review
unit-luminance = { $value } cd/m²
unit-images =
    { $value ->
       *[other] { $value } 张图像
    }
unit-threads =
    { $value ->
       *[other] { $value } 个线程
    }
unit-bytes =
    { $value ->
       *[other] { $value } 字节
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

## 分区

settings-section-general = 通用
settings-section-viewing = 查看
settings-section-browsing = 浏览
settings-section-colour = 色彩
settings-section-metadata = 元数据和隐私
# machine-drafted, needs native review
settings-section-privacy = 隐私
settings-section-performance = 性能
settings-section-accessibility = 辅助功能
settings-section-keys = 按键
settings-section-advanced = 高级

## 通用

settings-general-language = 界面语言
settings-general-language-help = 更改会立即生效。“系统默认”跟随操作系统的语言。
settings-general-region-formats = 区域格式
settings-general-region-formats-help = 日期、时间、数字和大小的书写方式。默认跟随界面语言。
settings-general-theme = 主题
settings-general-theme-help = 窗口和控件的浅色或深色外观。图像本身不会改变。
settings-general-theme-system = 跟随系统
settings-general-theme-light = 浅色
settings-general-theme-dark = 深色
# machine-drafted, needs native review
settings-general-on-start = { -app-name } 不带文件打开时
settings-general-on-start-help = 单独打开 { -app-name } 时显示的内容。从文件管理器或命令行打开的文件始终在查看器中显示。
settings-general-on-start-start-screen = 开始屏幕
settings-general-on-start-gallery = 上次文件夹的画廊
settings-general-on-start-library = 图库
settings-general-start-tips = 在开始屏幕上显示“试试这些”
settings-general-start-tips-help = 一行卡片，介绍实用功能及其按键。命令面板中的“显示欢迎页”可将其恢复。
settings-general-tips = 显示提示
settings-general-tips-help = 某项功能首次可能有用时显示一次简短提示，例如以画廊方式显示文件夹的按键。幻灯片放映期间从不显示。
settings-general-tips-shown = 已显示的提示
settings-general-tips-shown-help = 已经显示过的一次性提示，以免再次显示。
# machine-drafted, needs native review
settings-general-your-name = 你的姓名
settings-general-your-name-help = 显示在你添加的图章和标记上。仅保存在你创建的标记文件中。
settings-general-single-instance = 在现有窗口中打开文件
settings-general-single-instance-help = 当 { -app-name } 已在运行时，打开其他图像会在该窗口中显示，而不是启动新的实例。
settings-general-file-associations = 文件关联…
settings-general-file-associations-help = 将 { -app-name } 设为其支持的图像格式的默认查看器。

## 查看

settings-viewing-initial-zoom = 初始缩放
settings-viewing-initial-zoom-help = 新打开的图像以多大尺寸显示。
settings-viewing-initial-zoom-fit = 适合窗口
settings-viewing-initial-zoom-fit-if-larger = 大图像适合窗口，小图像以 { PERCENT(100) } 显示
settings-viewing-initial-zoom-actual-size = 实际大小（{ PERCENT(100) }）
settings-viewing-initial-zoom-fill = 填满窗口
settings-viewing-wheel-behaviour = 鼠标滚轮
settings-viewing-wheel-behaviour-help = 滚动滚轮时执行的操作。按住 Ctrl 可执行另一项操作。
settings-viewing-wheel-behaviour-help-macos = 滚动滚轮时执行的操作。按住 ⌘ 可执行另一项操作。
settings-viewing-wheel-behaviour-zoom = 缩放
settings-viewing-wheel-behaviour-navigate = 下一张和上一张图像
settings-viewing-keep-zoom = 在图像之间保持缩放
settings-viewing-keep-zoom-help = 放大时，下一张和上一张图像会以相同缩放打开并显示画面的同一部分，便于检查连拍的对焦。
settings-viewing-zoom-animation = 缩放动画
settings-viewing-zoom-animation-help = 平滑缩放，而不是直接跳变。开启“减少动态效果”时会自动关闭。
settings-viewing-zoom-animation-speed = 缩放动画速度
settings-viewing-zoom-animation-speed-help = 缩放动画完成的快慢。
settings-viewing-zoom-animation-speed-relaxed = 舒缓
settings-viewing-zoom-animation-speed-normal = 正常
settings-viewing-zoom-animation-speed-fast = 快速
settings-viewing-background = 背景
settings-viewing-background-help = 窗口中图像背后的颜色。
settings-viewing-background-theme = 与主题一致
settings-viewing-background-black = 黑色
settings-viewing-background-dark-grey = 深灰色
settings-viewing-background-light-grey = 浅灰色
settings-viewing-background-white = 白色
settings-viewing-background-custom = 自定颜色
settings-viewing-background-color = 自定背景颜色
settings-viewing-background-color-help = 当“背景”设为“自定颜色”时使用。
settings-viewing-fullscreen-background = 全屏背景
settings-viewing-fullscreen-background-help = 全屏时图像背后的颜色。
settings-viewing-transparency = 透明区域
settings-viewing-transparency-help = 图像透明部分的绘制方式。
settings-viewing-transparency-checkerboard = 棋盘格
settings-viewing-transparency-background = 背景颜色
settings-viewing-transparency-black = 黑色
settings-viewing-transparency-white = 白色
settings-viewing-pixel-grid-threshold = 显示像素网格的起始缩放
settings-viewing-pixel-grid-threshold-help = 放大到至少此比例时，在像素之间绘制网格。
settings-viewing-smoothing-high-zoom = 放大的像素
settings-viewing-smoothing-high-zoom-help = 图像以大于 { PERCENT(100) } 显示时像素的外观。
settings-viewing-smoothing-high-zoom-auto = 平滑，{ PERCENT(400) } 起锐利
settings-viewing-smoothing-high-zoom-smooth = 始终平滑
settings-viewing-smoothing-high-zoom-pixelated = 始终为锐利方块
settings-viewing-double-click = 双击图像
settings-viewing-double-click-help = 双击空白背景始终切换全屏。
settings-viewing-double-click-toggle-zoom = 在适合窗口和 { PERCENT(100) } 之间切换
settings-viewing-double-click-fullscreen = 切换全屏
settings-viewing-double-click-nothing = 无操作
settings-viewing-fullscreen-on-open = 以全屏打开图像
settings-viewing-fullscreen-on-open-help = 从 { -app-name } 外部打开图像时以全屏启动。
settings-viewing-fullscreen-mode = 全屏样式
settings-viewing-fullscreen-mode-help = 原生全屏使用独立的空间；即时全屏覆盖当前空间，没有动画。
settings-viewing-fullscreen-mode-native = 原生（独立空间）
settings-viewing-fullscreen-mode-borderless = 即时
settings-viewing-toolbar-autohide = 自动隐藏工具栏
settings-viewing-toolbar-autohide-help = 指针静止时，工具栏和边缘箭头会淡出，指针移动时重新出现。
settings-viewing-toolbar-autohide-delay = 隐藏工具栏的延迟
settings-viewing-toolbar-autohide-delay-help = 指针需要静止多久，工具栏才会淡出。
settings-viewing-toolbar-items = 工具栏按钮
settings-viewing-toolbar-items-help = 工具栏显示哪些按钮及其顺序。
settings-viewing-toolbar-drag = 拖动以重新排列工具栏
settings-viewing-toolbar-drag-help = 沿工具栏拖动按钮即可移动它。短暂单击仍会按下按钮；按 Esc 可将拖动中的按钮放回原处。
settings-viewing-toolbar-items-open = 打开
settings-viewing-toolbar-items-prev = 上一张图像
settings-viewing-toolbar-items-next = 下一张图像
settings-viewing-toolbar-items-zoom-out = 缩小
settings-viewing-toolbar-items-zoom-level = 缩放级别
settings-viewing-toolbar-items-zoom-in = 放大
settings-viewing-toolbar-items-fit = 适合窗口
settings-viewing-toolbar-items-rotate-left = 逆时针旋转
settings-viewing-toolbar-items-rotate-right = 顺时针旋转
settings-viewing-toolbar-items-info = 信息
settings-viewing-toolbar-items-copy = 复制图像
settings-viewing-toolbar-items-delete = 移到回收站
settings-viewing-toolbar-items-palette = 命令面板
settings-viewing-toolbar-items-settings = 设置
settings-viewing-toolbar-items-fullscreen = 全屏
# machine-drafted, needs native review
settings-viewing-toolbar-items-spacer = 弹性空白
settings-viewing-toolbar-items-separator = 分隔线
settings-toolbar-preview = 工具栏预览
settings-toolbar-hidden = 隐藏的按钮
settings-toolbar-add-spacer = 添加弹性空白
settings-toolbar-add-separator = 添加分隔线
settings-toolbar-reset = 恢复默认
settings-toolbar-remove = 移除{ $item }
settings-toolbar-hint = 拖动某行以移动它，或选中后按 Alt+↑ 或 Alt+↓。Tab 在搜索框和列表之间切换。
settings-toolbar-move-up = 上移
settings-toolbar-move-down = 下移
settings-toolbar-move-start = 移至开头
settings-toolbar-move-end = 移至末尾
settings-toolbar-menu-hide = 隐藏
settings-toolbar-menu-show = 显示
settings-toolbar-menu-remove = 移除
settings-toolbar-row-menu = { $item }的更多操作
settings-toolbar-position = 第 { $index } 个，共 { $count } 个
settings-toolbar-hidden-state = 已隐藏

## 浏览

settings-browsing-sort-order = 图像排序方式
settings-browsing-sort-order-help = 在文件夹中切换图像时使用的顺序。按名称排序时“img2”排在“img10”之前，并遵循你所用语言的排序规则。
settings-browsing-sort-order-natural = 名称
settings-browsing-sort-order-date-taken = 拍摄日期
settings-browsing-sort-order-modified = 修改日期
settings-browsing-sort-order-size = 文件大小
settings-browsing-sort-order-type = 文件类型
settings-browsing-sort-descending = 倒序
settings-browsing-sort-descending-help = 从最后一项到第一项排序。
settings-browsing-loop-folder = 到达文件夹末尾时循环
settings-browsing-loop-folder-help = 最后一张图像之后从第一张继续，反之亦然。
settings-browsing-include-subfolders = 包括子文件夹
settings-browsing-include-subfolders-help = 同时浏览子文件夹中的图像。
settings-browsing-hold-acceleration = 按住下一张或上一张时加速
settings-browsing-hold-acceleration-help = 按住按键或按钮时翻页加速的快慢。
settings-browsing-hold-acceleration-off = 不加速
settings-browsing-hold-acceleration-gentle = 平缓
settings-browsing-hold-acceleration-normal = 正常
settings-browsing-hold-acceleration-fast = 快速
settings-browsing-confirm-trash = 移到回收站前询问
settings-browsing-confirm-trash-help = 移到回收站的操作始终可以撤销，因此 { -app-name } 默认不会询问。
settings-browsing-arrows-follow-reading-direction = 方向键跟随阅读方向
settings-browsing-arrows-follow-reading-direction-help = 在从右到左书写的语言中，让左方向键转到下一张图像。

## 色彩

settings-colour-management = 色彩管理
settings-colour-management-help = 使用图像的颜色描述文件和显示器的描述文件，按原本的设计呈现颜色。
settings-colour-display-profile = 显示器描述文件
settings-colour-display-profile-help = 显示器的颜色描述文件。“自动”使用操作系统报告的描述文件。
settings-colour-rendering-intent = 渲染意图
settings-colour-rendering-intent-help = 如何将显示器无法呈现的颜色映射到其色域内。
settings-colour-rendering-intent-perceptual = 可感知
settings-colour-rendering-intent-relative-colorimetric = 相对比色
settings-colour-rendering-intent-saturation = 饱和度
settings-colour-rendering-intent-absolute-colorimetric = 绝对比色
settings-colour-black-point-compensation = 黑场补偿
settings-colour-black-point-compensation-help = 当显示器的黑色比图像的黑色更浅时，保留阴影细节。
settings-colour-untagged-images = 没有颜色描述文件的图像
settings-colour-untagged-images-help = 如何显示未注明所用色彩空间的图像。
settings-colour-untagged-images-srgb = 视为 sRGB
settings-colour-untagged-images-display = 不做更改直接显示
settings-colour-dithering = 抖动
settings-colour-dithering-help = 降低色深时添加细微的蓝噪声，以防止天空和渐变中出现色带。
settings-colour-hdr-output = HDR 输出
settings-colour-hdr-output-help = 在支持的显示器上以完整亮度显示 HDR 图像。自动模式会在显示器报告额外亮度时使用 HDR，否则会柔和地压缩高亮区域。
settings-colour-hdr-output-auto = 自动
settings-colour-hdr-output-on = 开
settings-colour-hdr-output-off = 关
# machine-drafted, needs native review
settings-colour-ocio-config = OCIO 配置
settings-colour-ocio-config-help = 用于 EXR、Radiance HDR 和 PFM 图像的 OpenColorIO 配置（.ocio）。留空则使用 $OCIO 指定的配置。
settings-colour-ocio-display = OCIO 显示设备
settings-colour-ocio-display-help = OCIO 配置中与你的显示器对应的显示设备。
settings-colour-ocio-display-default = 配置中的第一个
settings-colour-ocio-view = OCIO 视图
settings-colour-ocio-view-help = 场景线性（scene-linear）图像在该显示设备上的呈现方式，例如色调映射视图。
settings-colour-ocio-view-default = 显示设备的第一个
settings-colour-ocio-input = OCIO 输入色彩空间
settings-colour-ocio-input-help = 图像数值的色彩空间。
settings-colour-ocio-input-default = 配置的 scene-linear 角色
# machine-drafted, needs native review
settings-colour-proof-profile = 软打样配置文件
settings-colour-proof-profile-help = 冲洗中软打样上次使用的打印机配置文件。
settings-colour-proof-intent = 软打样渲染意图
settings-colour-proof-intent-help = 打印机无法打印的颜色如何处理。
settings-colour-proof-intent-perceptual = 可感知
settings-colour-proof-intent-relative = 相对比色
settings-colour-proof-bpc = 软打样黑场补偿
settings-colour-proof-bpc-help = 将黑色映射到打印机的黑色。
settings-colour-proof-paper = 软打样纸张白色
settings-colour-proof-paper-help = 显示纸张本身的颜色。
settings-colour-proof-ink = 软打样墨水黑色
settings-colour-proof-ink-help = 显示墨水黑色有多暗。

## 元数据和隐私

settings-metadata-show-info-on-open = 打开时显示信息面板
settings-metadata-show-info-on-open-help = 每次打开图像时，都打开包含相机、日期和地点详情的面板。
settings-metadata-info-detail = 信息面板显示内容
settings-metadata-info-detail-help = “摘要”以通俗的语言说明最有用的详细信息；“所有字段”列出文件中存储的全部内容。
settings-metadata-info-detail-summary = 摘要
settings-metadata-info-detail-all = 所有字段
settings-metadata-reverse-geocoding = 显示 GPS 位置的地名
settings-metadata-reverse-geocoding-help = 从内置数据中查找最近的城镇。此功能离线工作：位置信息绝不会离开你的电脑。
settings-metadata-hide-gps = 在屏幕上隐藏 GPS 位置
settings-metadata-hide-gps-help = 不显示坐标或地点，例如在共享屏幕时。文件不会被更改。
# machine-drafted, needs native review
settings-metadata-develop-sidecar = 将修改照片的编辑保存在照片旁边
# machine-drafted, needs native review
settings-metadata-develop-sidecar-help = 在文件旁写入 IMG_1234.CR2.specular.xmp，使编辑随文件一起移动。关闭：编辑仅保留在图库中。
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom = 在冲印中导入 Lightroom 设置
# machine-drafted, needs native review
settings-metadata-develop-import-lightroom-help = 带有 Lightroom 设置且没有编辑的照片会近似地按这些设置打开。关闭：冲印提供一个按钮。
settings-metadata-units = 单位
settings-metadata-units-help = 海拔、距离和速度的单位。
settings-metadata-units-auto = 跟随区域格式
settings-metadata-units-metric = 公制
settings-metadata-units-imperial = 英制
# machine-drafted, needs native review
settings-metadata-basemap = 底图
settings-metadata-basemap-help = 照片位置下方的地图。在线地图会下载你查看区域的图块。
settings-metadata-basemap-off = 仅内置地图（离线）
settings-metadata-basemap-openstreetmap = OpenStreetMap（在线）
settings-metadata-basemap-custom = 其他图块服务器（在线）
settings-metadata-basemap-file = 我自己的地图文件
settings-metadata-basemap-url = 图块服务器地址
settings-metadata-basemap-url-help = 包含 {"{"}z{"}"}、{"{"}x{"}"} 和 {"{"}y{"}"} 的地址，例如 https://tiles.example.org/{"{"}z{"}"}/{"{"}x{"}"}/{"{"}y{"}"}.png
settings-metadata-basemap-path = 地图文件
settings-metadata-basemap-path-help = MBTiles 或 PMTiles 文件。可离线使用。
settings-metadata-basemap-cache = 地图图块缓存
settings-metadata-basemap-cache-help = 下载图块所用的磁盘空间。最早的会先被移除。
# machine-drafted, needs native review
settings-metadata-basemap-allowed = 允许的图块服务器
settings-metadata-basemap-allowed-help = 你已同意从中下载图块的地图服务器。

## 性能

settings-performance-ram-cache = 内存缓存
settings-performance-ram-cache-help = 用于保存已解码图像以备随时显示的内存。“自动”会根据电脑的内存大小进行选择。
settings-performance-vram-cache = GPU 显存缓存
settings-performance-vram-cache-help = 用于图像和图块的 GPU 内存。“自动”会根据你的 GPU 进行选择。
settings-performance-prefetch-depth = 预先准备的图像数
settings-performance-prefetch-depth-help = 沿你的浏览方向预先解码多少张图像。
settings-performance-decoder-threads = 解码线程
settings-performance-decoder-threads-help = 用于解码图像的 CPU 线程。“自动”使用所有性能核心，并根据文件进行调整。
settings-performance-gpu-adapter = GPU
settings-performance-gpu-adapter-help = 当电脑有多个 GPU 时使用哪一个。
settings-performance-gpu-adapter-high-performance = 最高性能
settings-performance-gpu-adapter-low-power = 最低功耗
settings-performance-processing = 图像处理位置
settings-performance-processing-help = 部分工作可以在 CPU、GPU 或硬件解码器上运行。自动模式会测量这台电脑，为正在查看的图像选择最快的方式，为后台工作选择最省资源的方式。 这台电脑上选择了什么以及原因，见下方。
settings-performance-processing-automatic = 自动
settings-performance-processing-prefer-gpu = 优先使用 GPU
settings-performance-processing-cpu-only = 仅 CPU
settings-performance-drag-preview = 拖动时的预览
settings-performance-drag-preview-help = 拖动滑块时修改照片如何显示照片。自动仅在这台电脑跟不上时使用较小的画面。
settings-performance-drag-preview-automatic = 自动
settings-performance-drag-preview-sharp = 始终清晰
settings-performance-drag-preview-fast = 始终快速

## Settings → Performance → How { -app-name } runs on this computer (DESIGN.md §12.3.1)
# machine-drafted, needs native review
settings-paths-title = { -app-name } 在这台电脑上的运行方式
settings-paths-intro = { -app-name } 会在这台电脑上为每项工作的各种方式计时，并使用最好的一种。时间按 { $mpx } 百万像素的照片计算。
settings-paths-intro-measuring = 尚未测量完毕：{ -app-name } 会在电脑空闲时测量。
settings-paths-intro-battery = 使用电池时：优先选择耗能更低的方式。
settings-paths-keywords = GPU CPU NPU 显卡 硬件 加速 解码器 基准测试 速度
paths-engine-cpu = CPU
paths-engine-cpu-parallel = CPU（所有核心）
paths-engine-gpu = GPU
paths-engine-hw-decoder = 硬件解码器
paths-engine-npu = NPU
settings-paths-your-setting = { $engine }（您的设置）
settings-paths-measured-faster = 在这台电脑上测得：速度是{ $other }的 { $ratio } 倍。
settings-paths-measured-similar = 在这台电脑上测得：速度与{ $other }相当。
settings-paths-measured-power = 使用电池时，这种方式耗能最低（在这台电脑上测得）。
settings-paths-not-measured = 尚未测量：这类电脑的常规选择。
settings-paths-set-by = 由您的设置“{ $setting }：{ $option }”决定。
settings-paths-blocked-check = { $engine }已于 { $date } 停用：其结果与 CPU 的结果不一致。
settings-paths-blocked-error = { $engine }已于 { $date } 因出错而停用。
settings-paths-blocked-lost = { $engine }已于 { $date } 因停止响应而停用。
settings-paths-blocked-undated = { $engine }因出错而停用。
settings-paths-checking = { $engine }在用于正在查看的图像之前，会先与 CPU 的结果比对。
settings-paths-exploring = 正在后台试用{ $engine }。
settings-paths-unavailable = 这台电脑没有可用于此项工作的{ $engine }。
settings-paths-background-cheaper = 后台：{ $engine }，测得 CPU 时间少 { $percent }。
settings-paths-background-power = 后台：{ $engine }，以节省能耗。
settings-paths-background = 后台：{ $engine }。
settings-paths-detail-measured = { $engine }：{ $time } 毫秒，CPU 时间 { $cpu } 毫秒，{ $runs ->
   *[other] 测量 { $runs } 次
}（{ $confidence }）
settings-paths-detail-guess = { $engine }：约 { $time } 毫秒（估计）
settings-paths-confidence-low = 可信度低
settings-paths-confidence-medium = 可信度中等
settings-paths-confidence-high = 可信度高
settings-paths-machine = 测量对象：{ $gpu } · { $driver } · { $cpu }
settings-paths-retry = 重试
settings-paths-measure-again = 重新测量
settings-paths-measure-again-question = 清除在这台电脑上测得的结果（包括已停用的方式），并在电脑空闲时重新测量？
settings-paths-measure-again-confirm = 清除测量结果

settings-performance-display-cache = 在磁盘上保留显示缓存
settings-performance-display-cache-help = 存储与屏幕大小相当的图像副本，让看过的文件夹能够立即打开。
settings-performance-display-cache-size = 显示缓存大小
settings-performance-display-cache-size-help = 缓存已满时会移除最早的条目。
settings-performance-display-cache-location = 显示缓存位置
settings-performance-display-cache-location-help = 最好使用速度较快的驱动器上的文件夹。
settings-performance-fast-simd = 使用 SIMD 指令
settings-performance-fast-simd-help = 使用 CPU 的向量指令进行解码。仅在诊断问题时关闭。
settings-performance-fast-dct-scaling = 按屏幕尺寸解码
settings-performance-fast-dct-scaling-help = 直接以显示尺寸解码大型 JPEG。仅在诊断问题时关闭。
settings-performance-fast-parallel-decode = 多核解码
settings-performance-fast-parallel-decode-help = 将大型图像分配到多个 CPU 核心上解码。仅在诊断问题时关闭。
settings-performance-fast-restart-index = 为大型 JPEG 建立缩放索引
settings-performance-fast-restart-index-help = 记住 JPEG 各部分的起始位置，以便快速解码放大后的区域。仅在诊断问题时关闭。
settings-performance-fast-yuv-upload = 在 GPU 上转换颜色
settings-performance-fast-yuv-upload-help = 在 GPU 或 CPU 上转换 JPEG 颜色。自动模式选择在这台电脑上更快的一方。仅在诊断问题时关闭。
settings-performance-fast-yuv-upload-auto = 自动
settings-performance-fast-yuv-upload-on = 开
settings-performance-fast-yuv-upload-off = 关
settings-performance-fast-async-io = 异步读取文件
settings-performance-fast-async-io-help = 使用操作系统最快的文件接口预先读取。仅在诊断问题时关闭。
settings-performance-fast-mmap = 内存映射文件
settings-performance-fast-mmap-help = 无需复制即可读取文件。仅在诊断问题时关闭。
settings-performance-hardware-decode = 硬件解码
settings-performance-hardware-decode-help = 使用 GPU 内置的解码器打开 JPEG 照片。自动模式在有帮助时使用它，通常用于后台准备的图像。它会先与常规解码器的结果比对，无法处理的图像按常规方式打开。
settings-performance-hardware-decode-auto = 自动
settings-performance-hardware-decode-on = 开
settings-performance-hardware-decode-off = 关
settings-performance-frame-time-hud = 帧时间 HUD
settings-performance-frame-time-hud-help = 显示解码时间、帧时间以及正在使用的快速路径。

## 辅助功能

settings-accessibility-reduce-motion = 减少动态效果
settings-accessibility-reduce-motion-help = 将缩放、滑动和淡入淡出动画替换为即时切换。
settings-accessibility-reduce-motion-system = 跟随系统
settings-accessibility-reduce-motion-on = 开
settings-accessibility-reduce-motion-off = 关
settings-accessibility-increase-contrast = 增强对比度
settings-accessibility-increase-contrast-help = 更醒目的边框和文字，更少的半透明效果。
settings-accessibility-increase-contrast-system = 跟随系统
settings-accessibility-increase-contrast-on = 开
settings-accessibility-increase-contrast-off = 关
settings-accessibility-text-scale = 文字大小
settings-accessibility-text-scale-help = 文字相对于标准大小的尺寸。
settings-accessibility-ui-scale = 界面缩放
settings-accessibility-ui-scale-help = 整个界面的大小，在操作系统设置的显示缩放基础上叠加。
settings-accessibility-focus-ring-always = 始终显示焦点环
settings-accessibility-focus-ring-always-help = 即使在使用鼠标时，也显示哪个控件拥有键盘焦点。
settings-accessibility-screen-reader-verbosity = 屏幕阅读器详细程度
# machine-drafted, needs native review
settings-accessibility-screen-reader-verbosity-help = 在图像之间切换时屏幕阅读器播报内容的多少，以及它朗读光标下读数的多少。
settings-accessibility-screen-reader-verbosity-concise = 简略
settings-accessibility-screen-reader-verbosity-normal = 正常
settings-accessibility-screen-reader-verbosity-verbose = 详细
# machine-drafted, needs native review
settings-accessibility-announce-cursor = 朗读光标下的值
settings-accessibility-announce-cursor-help = 光标或键盘十字线停止移动后，屏幕阅读器会朗读其下的像素值、HU、温度或位置。

## 隐私

# machine-drafted, needs native review
settings-privacy-no-network = 无网络
# machine-drafted, needs native review
settings-privacy-no-network-help = 关闭：只有以下各项会联网，且每项都只在你开启它或用它打开内容之后：检查更新、下载本地 AI 模型、在线地图、来自网址或云存储的图像，以及你添加的服务器（DICOMweb、OMERO、地图）。开启：不会联网。

## 高级

settings-advanced-portable-mode = 便携模式
settings-advanced-portable-mode-help = 将设置和缓存保存在程序所在的文件夹中，而不是用户配置文件中。
settings-advanced-log-level = 日志详细程度
settings-advanced-log-level-help = { -app-name } 向日志文件写入多少内容。
settings-advanced-log-level-error = 仅错误
settings-advanced-log-level-warn = 警告
settings-advanced-log-level-info = 信息
settings-advanced-log-level-debug = 调试
settings-advanced-log-level-trace = 全部
settings-advanced-developer-mode = 开发者模式
settings-advanced-developer-mode-help = 显示用于开发和测试 { -app-name } 的工具，例如语言列表中的伪语言。
# machine-drafted, needs native review
settings-advanced-audit-log = 本地审计日志
# machine-drafted, needs native review
settings-advanced-audit-log-help = 将打开的医学图像、导出、匿名化、打印和发送到 PACS 的操作记录到数据文件夹中的 audit.jsonl。日志仅保存在本机。
settings-advanced-hw-report = 硬件报告…
settings-advanced-hw-report-help = 显示 { -app-name } 检测到的 CPU 和 GPU 功能，以及它使用的快速路径。
# machine-drafted, needs native review
settings-advanced-renderer-info = 渲染器信息…
settings-advanced-renderer-info-help = 显示 { -app-name } 使用的显卡适配器、驱动程序、显示格式和内存限制，可直接复制到错误报告中。
settings-advanced-edit-settings-file = 编辑 settings.toml
settings-advanced-edit-settings-file-help = 在 { -app-name } 内置编辑器中编辑设置文件。编辑器会在输入时检查文件,只有在没有错误时才保存。
settings-advanced-import = 导入设置…
settings-advanced-import-help = 从 { -app-name } 导出的文件中载入设置。
settings-advanced-export = 导出设置…
settings-advanced-export-help = 将所有设置保存到一个文件中，用于备份或在其他电脑上使用。
settings-advanced-reset-all = 重置所有设置…
settings-advanced-reset-all-help = 将所有设置恢复为默认值。此操作可以撤销。

# Stage 1 merge (machine-drafted, needs native review)
settings-performance-stall-warning = 加载停滞报告时间
settings-performance-stall-warning-help = 如果图像在此时间后仍在加载，则将其加载状态和待处理任务写入日志（标准错误输出或 SPECULAR_LOG_FILE）。
# machine-drafted, needs native review
settings-performance-insights = 性能提示
settings-performance-insights-help = 当图像因同一原因反复打开缓慢时，在角落显示“为什么这么慢？”及改进方法。所了解的信息只保留在此计算机上。
settings-performance-insights-storage = 关于存储较慢的提示
settings-performance-insights-storage-help = 当读取文件（来自网络共享、慢速驱动器或归档）反复拖慢图像打开时显示提示。
settings-performance-insights-decoder = 关于解码的提示
settings-performance-insights-decoder-help = 当图像反复在 CPU 上解码、而硬件解码器本可帮上忙时显示提示。
# machine-drafted, needs native review
settings-performance-insights-restore = 重新显示已隐藏的提示
settings-performance-insights-restore-help = 恢复你在“为什么这么慢？”中用 × 隐藏的提示，包括打开图像和缩略图两方面。没有隐藏的提示时不可用。

## settings.toml editor (machine-drafted, needs native review)

config-editor-title = settings.toml
config-editor-save = 保存
config-editor-revert = 还原
config-editor-close = 关闭
config-editor-modified = 已修改
config-editor-text = 设置文件
config-editor-position = 第 { $line } 行,第 { $column } 列
config-editor-saved = 已保存并应用 settings.toml。
config-editor-cant-save = 无法保存。需要修复的问题:{ $count } 个。
config-editor-save-failed = 无法保存 settings.toml:{ $reason }
config-editor-conflict = settings.toml 已在别处被修改。
config-editor-reload = 重新加载
config-editor-keep-mine = 保留我的版本
config-editor-discard-question = 放弃对 settings.toml 的未保存更改?
config-editor-discard = 放弃
config-editor-keep-editing = 继续编辑
config-editor-find-placeholder = 在文件中查找
config-editor-find-count = { $current } / { $total }
config-editor-find-none = 无匹配项
config-editor-find-previous = 上一个匹配项
config-editor-find-next = 下一个匹配项
config-editor-find-close = 关闭查找
config-editor-problems = 问题
config-editor-problems-count =
    { $count ->
        [0] 没有问题
       *[other] 问题:{ $count } 个
    }
config-editor-no-problems = 没有问题。可以保存文件。
config-editor-problem-at = 第 { $line } 行:{ $message }
config-editor-severity-error = 错误
config-editor-severity-warning = 警告
config-editor-severity-info = 提示
config-editor-problem-syntax = 无效的 TOML:{ $reason }
config-editor-problem-duplicate = { $key } 在同一个表中设置了两次。请删除其中一个。
config-editor-problem-unknown-section = 没有设置分区 [{ $key }]。
config-editor-problem-unknown-section-suggestion = 没有设置分区 [{ $key }]。您是指 [{ $suggestion }] 吗?
config-editor-problem-unknown-key = 没有设置项 { $key }。
config-editor-problem-unknown-key-suggestion = 没有设置项 { $key }。您是指 { $suggestion } 吗?
config-editor-problem-invalid = { $key } 的值无效:{ $reason }
config-editor-problem-migrated = { $key } 采用的是旧版 { -app-name } 的写法。读取时会自动转换。
config-editor-problem-newer-version = 此文件由更新版本的 { -app-name } 写入(格式 { $found };此版本可读取 { $supported })。未知设置会被保留。
config-editor-problem-command = [[commands]] 中的命令 { $index }:{ $reason }
config-editor-problem-hook = [[hooks]] 中的钩子 { $index }:{ $reason }

## keymap.toml in the same editor (machine-drafted, needs native review)

config-editor-keymap-title = keymap.toml
config-editor-keymap-text = 按键绑定文件
config-editor-keymap-saved = keymap.toml 已保存并应用。
config-editor-keymap-save-failed = 无法保存 keymap.toml：{ $reason }
config-editor-keymap-conflict = keymap.toml 已在其他地方更改。
config-editor-keymap-discard-question = 放弃对 keymap.toml 的未保存更改吗？
config-editor-new-file = 新文件，保存时创建
config-editor-no-folder = 没有可保存到的设置文件夹。

## Reset all settings confirmation (machine-drafted, needs native review)

settings-reset-all-confirm = 将所有设置恢复为默认值吗？
settings-reset-all-confirm-detail = 这会影响设置的所有部分。之后可以撤销。
settings-reset-all-confirm-ok = 全部重置
settings-reset-all-done = 所有设置已恢复为默认值。

## Hardware video encoding (machine-drafted, needs native review)
paths-engine-hw-encoder = 硬件编码器

## Path timing units (machine-drafted, needs native review)
settings-paths-checking-before-use = { $engine }在使用前会先与 CPU 的结果比对。
settings-paths-unit-thumbnail = 时间按每张 { $size } 缩略图计算
settings-paths-unit-frame = 时间按每个 { $size } 视频帧计算
# machine-drafted, needs native review
settings-metadata-develop-calibration = 校准帧文件夹
# machine-drafted, needs native review
settings-metadata-develop-calibration-help = 用于冲印的平场帧和暗场帧。文件名或文件夹名包含“flat”或“dark”。

## Colour swatches under a colour setting's field (machine-drafted, needs native review)
settings-colour-white = 白色
settings-colour-light-grey = 浅灰色
settings-colour-grey = 灰色
settings-colour-dark-grey = 深灰色
settings-colour-black = 黑色
settings-colour-yellow = 黄色
settings-colour-orange = 橙色
settings-colour-red = 红色
settings-colour-magenta = 品红
settings-colour-blue = 蓝色
settings-colour-cyan = 青色
settings-colour-green = 绿色
