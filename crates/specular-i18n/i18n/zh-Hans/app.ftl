# machine-drafted, needs native review
# Specular — 查看器界面、命令面板、错误信息和“关于”窗口。简体中文。

-app-name = Specular

## 工具栏提示和辅助功能标签

toolbar-open = 打开图像…
toolbar-prev = 上一张图像
toolbar-next = 下一张图像
toolbar-back-gallery = 返回画廊
toolbar-zoom-out = 缩小
toolbar-zoom-in = 放大
toolbar-zoom-level = 缩放级别和预设
toolbar-zoom-fit = 适合窗口
toolbar-zoom-100 = 实际大小（{ PERCENT(100) }）
toolbar-rotate-left = 逆时针旋转
toolbar-rotate-right = 顺时针旋转
toolbar-fullscreen-enter = 进入全屏
toolbar-fullscreen-exit = 退出全屏
toolbar-info = 图像信息
toolbar-settings = 设置
toolbar-delete = 移到回收站
toolbar-copy = 复制图像
toolbar-palette = 命令面板
toolbar-go-to = 转到图像…
toolbar-go-to-range = 1–{ $count }
# machine-drafted, needs native review
toolbar-customise = 自定义工具栏…
toolbar-more = 更多
toolbar-announce-moved = { $item }：第 { $index } 个，共 { $count } 个
toolbar-announce-added = 已添加{ $item }：第 { $index } 个，共 { $count } 个
toolbar-announce-removed = 已从工具栏移除{ $item }
toolbar-announce-reset = 工具栏已恢复默认
tooltip-with-shortcut = { $label }（{ $shortcut }）

## 读数

readout-position = { $index } / { $count }
readout-position-a11y = 第 { $index } 张图像，共 { $count } 张
readout-zoom = { $zoom }
readout-zoom-a11y = 缩放 { $zoom }
readout-zoom-fit = 适合（{ $zoom }）
zoom-preset-fit = 适合窗口
zoom-preset-fill = 填满窗口
zoom-preset-actual = 实际大小
# machine-drafted, needs native review
# The last entry of the zoom presets: the window fills the screen (ticked while it does).
zoom-preset-fullscreen = 全屏
zoom-preset-percent = { $zoom }
readout-dimensions = { $width } × { $height }
status-image-count =
    { $count ->
       *[other] { $count } 张图像
    }

## 命令面板

palette-placeholder = 输入命令或设置…
palette-no-results = 没有匹配的命令
palette-set-setting = 设置：{ $setting } → { $value }
palette-next = 下一张图像
palette-prev = 上一张图像
palette-first = 第一张图像
palette-last = 最后一张图像
palette-go-to = 转到图像…
palette-zoom-in = 放大
palette-zoom-out = 缩小
palette-zoom-fit = 缩放至适合
palette-zoom-fill = 缩放至填满
# machine-drafted, needs native review
palette-zoom-fit-width = 缩放至适合宽度
palette-zoom-fit-height = 缩放至适合高度
palette-zoom-100 = 缩放至 { PERCENT(100) }
palette-zoom-200 = 缩放至 { PERCENT(200) }
palette-zoom-300 = 缩放至 { PERCENT(300) }
palette-zoom-400 = 缩放至 { PERCENT(400) }
palette-zoom-rectangle = 缩放至所选区域
palette-rotate-left = 逆时针旋转
palette-rotate-right = 顺时针旋转
palette-flip-horizontal = 水平翻转
palette-flip-vertical = 垂直翻转
palette-toggle-fullscreen = 切换全屏
palette-navigator = 显示或隐藏导航器
palette-loupe = 放大镜
palette-pixel-grid = 显示或隐藏像素网格
palette-info-panel = 显示或隐藏信息面板
palette-copy-image = 复制图像
# machine-drafted, needs native review
palette-read-codes = 读取二维码
# machine-drafted, needs native review
palette-find-text = 查找文字
# machine-drafted, needs native review
palette-face-boxes = 显示人脸
palette-copy-path = 复制文件路径
palette-move-to-trash = 移到回收站
palette-undo-trash = 撤销移到回收站
palette-open = 打开图像…
# machine-drafted, needs native review
palette-open-address = 打开网址…
palette-settings = 设置
# machine-drafted, needs native review
palette-customise-toolbar = 自定义工具栏…
palette-cheat-sheet = 键盘快捷键
palette-hud = 显示或隐藏帧时间 HUD
palette-language = 语言
palette-theme = 主题
# machine-drafted, needs native review
palette-interface-larger = 放大界面
palette-interface-smaller = 缩小界面
palette-interface-reset = 界面恢复 { PERCENT(100) }
palette-quit = 退出 { -app-name }

## 快捷键速查表

cheatsheet-title = 键盘快捷键
cheatsheet-navigation = 浏览
cheatsheet-zoom-pan = 缩放和平移
cheatsheet-view = 显示
cheatsheet-file = 文件
cheatsheet-window = 窗口
cheatsheet-help = 帮助
cheatsheet-hold = 按住 { $key }
cheatsheet-close-hint = 按 { $key } 或 Esc 关闭

## 错误

# machine-drafted, needs native review
viewer-no-preview = 此文件内没有预览。
error-cannot-open = 无法打开 { $file }
error-cannot-open-reason = 无法打开 { $file }：{ $reason }
error-unsupported-format = { -app-name } 暂时无法读取 { $file } 的格式。
error-file-truncated = { $file } 不完整。正在显示可读取的部分。
error-file-too-large = { $file } 太大，无法打开（{ $megapixels } 百万像素；上限为 { $limit } 百万像素）。
error-not-found = { $file } 已不存在。
error-permission-denied = 你没有读取 { $file } 的权限。
error-details = 详细信息
crash-last-time = { -app-name } 上次意外关闭；报告已保存到 { $path }
error-trash-failed = 无法将 { $file } 移到回收站：{ $reason }
# machine-drafted, needs native review
# 显示在“无法打开 FILE：”及其他“无法……：”消息之后的原因。说明发生了什么，并在有帮助时说明该怎么做。
error-reason-not-found = 它已不存在。可能已被移动、重命名或删除。
error-reason-permission = 你没有读取它的权限。请检查其所有者和权限。
error-reason-disk-full = 磁盘已满。请释放一些空间后重试。
error-reason-read-only = 磁盘为只读。
error-reason-truncated = 文件不完整。它可能仍在复制或下载中。
error-reason-is-folder = 它是文件夹，不是文件。
# A remote store (a Zarr image on a server or in an S3 bucket).
error-reason-remote-denied = 服务器拒绝访问。数据可能是私有的；如果是 S3 存储桶，请先设置 AWS 凭据。
error-reason-remote-missing = 该地址没有 Zarr 数据。
# $status is an HTTP status code ("500").
error-reason-remote-status = 服务器返回了错误 { $status }。
error-reason-offline = 无法连接服务器。请检查地址和网络连接。
# machine-drafted, needs native review
error-reason-network-off = 网络已在设置 › 隐私中关闭。
error-reason-timed-out = 驱动器或服务器未及时响应。请检查连接后重试。
error-reason-memory = 内存不足。请关闭其他图像或应用程序后重试。
error-reason-unsupported = 它的格式 { -app-name } 暂时无法读取。
# $detail 是解码器自己的英文技术描述（"12-bit lossless JPEG"）。
error-reason-unsupported-detail = 它使用了 { -app-name } 暂时无法读取的功能（{ $detail }）。
# machine-drafted, needs native review
# A file whose extension several formats share (".img") and whose content is none of them.
# $formats lists the format names, e.g. "ERDAS IMAGINE, ADSC SMV, Analyze 7.5".
error-reason-not-one-of = 它不是 { -app-name } 可读取的任何一种 .{ $ext } 文件（{ $formats }）。
error-reason-damaged = 文件已损坏，或与其名称所示的格式不符（{ $detail }）。
error-reason-too-large = 它太大，无法打开。
# machine-drafted, needs native review
error-reason-needs-memory = 它需要 { $needed } 可用 RAM，但当前只有 { $free } 可用。请关闭其他一些应用后重试，或在内存更大的电脑上打开它。
error-needs-memory = 此图像需要 { $needed } 可用 RAM，但当前只有 { $free } 可用。请关闭其他一些应用后重试，或在内存更大的电脑上打开它。
error-needs-memory-short = 需要 { $needed } 可用 RAM
error-reason-cancelled = 打开操作已停止。
error-reason-trash-access = 它已不存在，或你没有更改它的权限。
error-reason-restore-collision = 其原文件夹中已有同名文件。
error-reason-trash-root = 无法将整个驱动器移到回收站。
error-reason-not-in-trash = 它已不在回收站中。
error-reason-program-missing = 该程序未安装或已被移动。请重新选择或重新安装。
error-reason-program-not-allowed = 不允许运行该程序。请检查它是否可执行。
# machine-drafted, needs native review
error-reason-permission-write = 你没有更改它的权限。请检查其所有者和权限。
error-reason-file-has-errors = 它有错误。请先修复，以免其中的内容丢失。
error-reason-not-dicom = 它不是 DICOM 文件。
error-reason-no-image = 它不包含图像。
error-reason-needs-layout = 它是无文件头的体数据。需要知道其尺寸、样本类型和字节顺序才能打开。
error-reason-not-archive = 它不是 { -app-name } 能读取的压缩包。
error-reason-needs-sandbox = 此类压缩包只能由 { -app-name } 的受保护读取器打开，而该读取器不可用。
error-reason-encrypted = 它已加密，需要密码。
error-reason-wrong-password = 密码错误。
error-reason-checksum = 文件已损坏：其内容与校验和不符。
error-reason-missing-entry = 其中的一个文件缺失。
error-reason-busy = 另一个程序正在使用它。请在该程序中关闭后重试。
error-reason-catalogue-damaged = 目录文件已损坏。
error-reason-cannot-open = 无法打开其文件。请检查其文件夹是否存在以及你是否有写入权限。
error-reason-newer-version = 它由较新版本的 { -app-name } 写入。请更新 { -app-name } 后再更改它。
error-reason-internal = { -app-name } 内部出错。详情见日志。
error-reason-volume-too-few = { $count ->
   *[other] { $count } 个层面太少，无法构建体数据。
}
error-reason-volume-no-geometry = 某个层面未说明其位置或朝向。
error-reason-volume-not-parallel = 层面互不平行。
error-reason-volume-shared-position = 多幅图像共享 { $count } 个位置中的每一个（时相、回波或时间序列）。
error-reason-volume-missing = 约缺少 { $count } 个层面。
error-reason-volume-mismatch = 层面的尺寸或像素间距不一致。
error-reason-volume-not-grey = 层面是彩色的，不是灰度值。
error-reason-volume-skewed = 体数据的各轴不互相垂直。
error-reason-volume-too-large = 它需要 { $need } MiB，但仅允许 { $budget } MiB。
error-reason-url-not-specular = 它不是 { -app-name } 链接。
error-reason-url-too-long = 它太长。
error-reason-url-damaged = 它已损坏。可能在复制时被截断或更改。
error-reason-url-unknown = 它请求了此版本 { -app-name } 不认识的内容。更新 { -app-name } 可能会有帮助。
error-reason-url-not-absolute = 其中的文件路径必须是完整路径。
error-reason-url-no-paths = 它未指定任何文件。
error-reason-url-too-many = 它指定的文件太多。

## 提示消息

# machine-drafted, needs native review
# A background task (a listing, an export, a file operation) stopped part way; $reason is
# error-reason-internal.
background-failed = 未能完成：{ $reason }
toast-moved-to-trash = 已移到回收站
toast-moved-to-trash-named = 已将 { $file } 移到回收站
toast-moved-to-trash-count =
    { $count ->
       *[other] 已将 { $count } 张图像移到回收站
    }
toast-undo = 撤销
toast-restored = 已恢复 { $file }
toast-copied-image = 已复制图像
toast-copied-path = 已复制路径
toast-copied-report = 已复制报告
toast-copied-value = 已复制值
toast-loop-start = 已回到第一张图像
toast-loop-end = 已跳到最后一张图像
# machine-drafted, needs native review
toast-interface-size = 界面大小 { PERCENT($percent) }
toast-language-changed = 语言已更改为 { $language }

## 空白状态

empty-drop = 将图像拖放到此处，或按 { $shortcut }
empty-open-button = 打开图像…
empty-drop-release = 松开以打开
empty-folder = 此文件夹中没有 { -app-name } 可以打开的图像。
# machine-drafted, needs native review
# 空窗口中“打开图像…”下方：在哪里找到其他功能。
empty-hint-keys = 按 { $cheatsheet } 查看键盘快捷键，或按 { $palette } 查找任意命令。
empty-hint-cheatsheet = 按 { $cheatsheet } 查看键盘快捷键。
empty-hint-palette = 按 { $palette } 查找任意命令。
# 文件打开需要一点时间时，显示在加载指示器下方。
viewer-opening = 正在打开 { $file }…
fullscreen-exit-hint = 按 Esc 退出全屏

## 语言选择器

language-picker-title = 语言
# machine-drafted, needs native review
language-picker-search = 搜索语言
language-completeness = 已翻译 { $percent }
language-system-default = 系统默认（{ $language }）
language-pseudo-accented = 伪区域设置：带重音且更长
language-pseudo-rtl = 伪区域设置：从右到左镜像

## “关于”窗口

about-title = 关于 { -app-name }
about-version = 版本 { $version }
about-tagline = 快速、忠实的图像查看器。
about-copyright = © { $year } { -app-name } 作者
about-license = 依据 MIT License / Apache License 2.0 授权。
about-geonames = 地名数据来自 GeoNames（geonames.org），依据 Creative Commons Attribution 4.0 授权。
about-unicode = 区域设置数据来自 Unicode CLDR，经由 ICU4X 提供。© Unicode, Inc.
# machine-drafted, needs native review
about-lensfun = 镜头配置文件来自 Lensfun 数据库（lensfun.github.io），依据 Creative Commons Attribution-ShareAlike 3.0 许可。
about-third-party = 第三方许可证
about-copy-info = 复制系统信息
about-close = 关闭

## 查看器界面（第 1 阶段）
# machine-drafted, needs native review
app-close = 关闭
app-truncated-badge = 文件不完整
app-info-summary = 摘要
app-info-all-fields = 所有字段
app-info-copy-value = 复制值
app-info-copy-field = 复制 { $field } 的值
app-info-loading = 正在读取元数据…
# machine-drafted, needs native review
# “所有字段”中，文件除像素外不含任何元数据时显示。
app-info-no-fields = 此文件没有元数据字段。
# Gain-map row of the info summary (machine-drafted, needs native review)
app-info-gain-map = HDR 增益图
app-info-gain-map-headroom = 最高可达 SDR 亮度的 { $headroom } 倍
app-copy-failed = 无法复制：{ $reason }
app-restore-failed = 无法恢复 { $file }：{ $reason }
app-open-dialog-title = 打开图像
app-open-dialog-filter = 支持的文件
app-open-dialog-all = 所有文件
app-open-group-images = 图像
app-open-group-raw = 相机 RAW
app-open-group-video = 视频
app-open-group-sound = 声音
app-open-group-documents = 文档
app-open-group-drawings = 图纸与 CAD
app-open-group-models = 3D 模型
app-open-group-maps = 地图与轨迹
app-open-group-science = 科学与医学
app-open-group-fonts = 字体
app-open-group-other = 其他

## 渲染器信息（关于、设置 › 高级）

# machine-drafted, needs native review
renderer-title = 渲染器信息
renderer-section-adapter = 显卡适配器
renderer-section-limits = 限制和功能
renderer-section-display = 显示器
renderer-section-memory = 内存
renderer-section-decoders = 可选解码器
renderer-unknown = 未知
# The PCI vendor and device IDs ("1002:747e").
renderer-ids = 供应商和设备 ID
# Wayland, X11, Windows or macOS.
renderer-display-server = 显示服务器
renderer-features = 使用的功能
renderer-surface-format = 窗口格式
renderer-surface-formats = 可用格式
renderer-present-mode = 呈现模式
renderer-present-modes = 可用模式
renderer-hdr = HDR
# $headroom is how much brighter than white the display goes ("4.0").
renderer-hdr-on = 开（{ $headroom }× 余量）
renderer-hdr-off = 关（显示器可以显示 HDR）
renderer-hdr-unavailable = 此显示器不可用
renderer-decode-budget = 解码可使用
renderer-decode-budget-detail = 可用内存的 { PERCENT(80) }，由所有正在打开的图像共享。
renderer-display-cache = 显示缓存

## 性能 HUD（F12）
# machine-drafted, needs native review
app-hud-scale = 缩放
app-hud-level = 级别
app-hud-level-thumbnail = 内嵌缩略图
app-hud-level-full = 完整分辨率
app-hud-level-scaled = 1/{ $denominator }
app-hud-open = 打开
app-hud-decode = 解码
app-hud-upload = 上传
app-hud-frame = 帧
app-hud-threads = 线程
app-hud-threads-value = { $threads }（{ $queued } 个排队中）
app-hud-tiles = 图块
app-hud-tiles-value = { $tiles } / { $capacity }
app-hud-ram = 内存
app-hud-vram = GPU 内存
app-hud-gpu = GPU

# Trash confirmation (machine-drafted, needs native review)
app-confirm-trash-title = 将 { $file } 移到回收站？
# machine-drafted, needs native review
app-confirm-trash-sidecars = 它的附属文件也会移到回收站：{ $files }
app-cancel = 取消

# Stage 1 merge (machine-drafted, needs native review)
disabled-no-image = 未打开图像
disabled-still-loading = 图像仍在加载
disabled-decode-failed = 无法解码此图像
disabled-no-other-images = 此文件夹中没有其他图像
tooltip-unavailable = { $label }（不可用：{ $reason }）
error-decoder-stopped = 解码器已停止，未生成图像
# machine-drafted, needs native review
# Shown after "Can’t open FILE:" for brain data (GIFTI .func/.shape/.label, CIFTI) that is
# drawn on a surface: no .surf.gii with that many vertices was found in the same folder.
error-needs-surface = 同一文件夹中需要有大脑表面（含 { $vertices } 个顶点的 .surf.gii 文件）
fullscreen-back-hint = 按 Esc 重置缩放或退出全屏
action-back = 关闭面板（在对话框中先后退一步）、重置缩放、退出全屏；在画廊中，向上一级文件夹
about-third-party-file = { -app-name } 包含按其各自许可证提供的第三方软件和数据；请参阅随 { -app-name } 一起安装的 THIRD-PARTY.md。
toast-low-memory = 此图像可能需要约 { $need } 内存，但只有 { $available } 可用。加载可能很慢或失败。

hw-report-title = 硬件报告
hw-report-copy = 复制报告
hw-report-version = 版本
hw-report-cpu = CPU
hw-report-cores = 核心
hw-report-cores-value = { $logical } 个逻辑，{ $physical } 个物理
hw-report-performance-cores = 性能核心
hw-report-cpu-features = CPU 加速
hw-report-none = 无
hw-report-memory = 内存
hw-report-memory-value = 共 { $total }，可用 { $available }
hw-report-graphics-api = 图形 API
hw-report-driver = 驱动程序
hw-report-max-texture = 最大纹理
hw-report-pixels = { $width } × { $height } 像素
hw-report-decode-threads = 解码线程
hw-report-ram-cache = 图像内存缓存
hw-report-vram-cache = GPU 内存缓存
hw-report-processing = 处理
hw-report-path-jpeg-decode = JPEG 解码
hw-report-path-ycbcr-to-rgb = 颜色转换
hw-report-path-png-unfilter = PNG 解码
hw-report-path-heic-decode = HEIC 解码
hw-report-path-inference = 图像分析
hw-report-engine-cpu = CPU
hw-report-engine-cpu-parallel = CPU（并行）
hw-report-engine-gpu = GPU
hw-report-engine-hw-decoder = 硬件解码器
hw-report-engine-npu = NPU
# machine-drafted, needs native review
hw-report-build-debug = 调试版
hw-report-build-release = 发布版

## Accessibility (screen readers)
# The image as a screen reader announces it: file name and size in pixels.
a11y-image = { $name }，{ $width } × { $height } 像素
# The region where short notifications (toasts) appear.
a11y-notifications = 通知
# machine-drafted, needs native review
a11y-transport-playback = 播放
a11y-transport-pages = 页面
a11y-settings-sections = 分区

## Read codes (QR codes in the picture) (machine-drafted, needs native review)
codes-placeholder = 此图像中的二维码
codes-looking = 正在查找二维码…
codes-none = 此图像中未找到二维码。
codes-failed = 无法读取图像：{ $reason }
codes-copy = 复制“{ $text }”
# $kind is "QR", $version a number.
codes-kind = { $kind } { $version }
# Data Matrix: its size in modules, e.g. "Data Matrix 12×12".
# machine-drafted, needs native review
codes-kind-size = { $kind } { $rows }×{ $cols }
codes-open-row = 打开 { $host } 上的链接…
codes-network-off = 未开启任何网络，因此无法打开链接。
codes-open-title = 打开指向 { $host } 的链接？
codes-open-detail = 链接将在网页浏览器中打开。只打开你信任的链接。
codes-open = 在浏览器中打开
# machine-drafted, needs native review
# Find text: the text in the picture shown, read by the Local AI pack's Text models.
ocr-placeholder = 此图像中的文字
# machine-drafted, needs native review
ocr-looking = 正在识别文字…
# machine-drafted, needs native review
ocr-none = 此图像中未找到文字。
# machine-drafted, needs native review
ocr-failed = 无法识别文字：{ $reason }
# machine-drafted, needs native review
ocr-needs-pack = “查找文字”需要本地 AI 包中的文字模型（设置 › 本地 AI）。
# machine-drafted, needs native review
ocr-loading = 正在加载文字模型…
# machine-drafted, needs native review
# A row: choosing it copies the line. $text is the line (one line, shortened).
ocr-copy = 复制“{ $text }”
# machine-drafted, needs native review
ocr-copy-all = 复制全部文字
# machine-drafted, needs native review
# A row's note: how sure the reading is, and the add-on that read it. $percent is formatted.
ocr-confidence = 置信度 { $percent }

## Hardware report panel (machine-drafted, needs native review)
hw-report-section-system = 系统
hw-report-section-gpu = GPU
hw-report-section-decoding = 解码
hw-report-operating-system = 操作系统
hw-report-adapter = 适配器
hw-report-gpu-type = 类型
hw-report-gpu-init = 启动时间
hw-report-max-layers = 纹理层数
hw-report-compression = 纹理压缩
hw-report-unified-memory = 与 CPU 共享内存
hw-report-display = 显示器
hw-report-hw-decode = 硬件解码
hw-report-core-cluster = 核心组
hw-report-power = 电源
hw-report-calibration = 速度测量
hw-report-codec-others = 其他
hw-report-performance-cores-value = { $total } 个中的 { $count } 个
hw-report-cluster-value = { $name }：{ $physical } 个物理，{ $logical } 个逻辑
hw-report-thermal-nominal = 温度正常
hw-report-thermal-fair = 偏热
hw-report-thermal-serious = 较热，正在降频
hw-report-thermal-critical = 严重过热
hw-report-power-battery = 使用电池（{ $percent }）
hw-report-power-battery-unknown = 使用电池
hw-report-power-mains = 已接通电源
hw-report-power-mains-percent = 已接通电源（{ $percent }）
hw-report-power-saving = 节能模式已开启
hw-report-gpu-discrete = 独立
hw-report-gpu-integrated = 集成
hw-report-gpu-virtual = 虚拟
hw-report-gpu-software = 软件，在 CPU 上运行
hw-report-gpu-other = 其他
hw-report-gpu-unavailable = 不可用
hw-report-ms = { $ms } 毫秒
hw-report-yes = 是
hw-report-no = 否
hw-report-unified-platform-differs = 系统报告的情况不同
hw-report-display-value = { $fps } Hz，HDR 余量当前 { $now }，最高 { $max }
hw-report-hw-decode-value = { $device } 上的 { $api }
hw-report-hw-decode-checking = 仍在检查…
hw-report-codec-up-to = 最高 { $width } × { $height }
hw-report-codec-none = 不可用
hw-report-codec-h264-encode = H.264 编码
hw-report-supported = 支持：{ $list }
hw-report-unsupported = 不支持：{ $list }
hw-report-calibration-done = 已在这台电脑上测量
hw-report-calibration-pending = 尚未测量
hw-report-paths-power-saving = 节能中：优先选择耗能更低的方式
hw-report-paths-caption = { $mpx } 百万像素图像的处理方式
hw-report-col-task = 任务
hw-report-col-path = 方式
hw-report-col-time = 时间
hw-report-col-cpu = CPU 时间（所有线程合计）
hw-report-col-runs = 测量次数
hw-report-col-use = 用途
hw-report-runs-untested = 未测试
hw-report-use-viewing = 查看（{ $reason }）
hw-report-use-background = 后台（{ $reason }）
hw-report-reason-only = 唯一选项
hw-report-reason-faster = 最快
hw-report-reason-cheaper = CPU 时间最少
hw-report-reason-lower-power = 耗能最少
hw-report-reason-explore = 正在试用
hw-report-reason-forced = 在设置中指定
hw-report-reason-prefer-gpu = 优先使用 GPU
hw-report-reason-cpu-only = 仅 CPU
hw-report-reason-calibrate = 正在测量
hw-report-reason-scaled = 缩小尺寸
hw-report-trust-unverified = 尚未检查
hw-report-trust-blocked = 因出错已停用
hw-report-trust-absent = 这台电脑上没有

## Keyboard cheat sheet tabs and sections (machine-drafted, needs native review)
cheatsheet-mode-gallery = 画廊
cheatsheet-mode-compare = 比较
cheatsheet-gallery-browse = 浏览
cheatsheet-gallery-select = 选择
cheatsheet-gallery-mark = 评分和旗标
cheatsheet-gallery-move = 在图像之间移动
cheatsheet-gallery-page = 向上或向下翻页
cheatsheet-gallery-ends = 第一张或最后一张图像
cheatsheet-gallery-up = 返回上一级文件夹
cheatsheet-gallery-history-back = 返回上一个文件夹
cheatsheet-gallery-history-forward = 前进到下一个文件夹
cheatsheet-gallery-back = 关闭最上层视图，然后返回
cheatsheet-gallery-viewer = 返回查看器
cheatsheet-gallery-album-add = 添加到上次使用的相册
cheatsheet-gallery-select-one = 选择或取消选择
cheatsheet-gallery-quick-look = 快速查看
cheatsheet-gallery-filter = 筛选
cheatsheet-gallery-bigger = 增大缩略图
cheatsheet-gallery-smaller = 缩小缩略图
cheatsheet-gallery-default-size = 标准缩略图大小
cheatsheet-gallery-refresh = 刷新文件夹
cheatsheet-gallery-menu = 右键菜单
cheatsheet-compare-views = 比较
cheatsheet-compare-overlay = 叠加或并排
cheatsheet-compare-difference = 差异（再按：下一种）
cheatsheet-compare-step-down = 降低混合或差异增益
cheatsheet-compare-step-up = 提高混合或差异增益
cheatsheet-compare-pick = 按键作用的图像
cheatsheet-compare-close = 关闭比较
cheatsheet-channels = 通道
cheatsheet-medical-window = 窗宽/窗位
cheatsheet-medical-measure = 测量和标注
cheatsheet-medical-frames = 帧和序列
# machine-drafted, needs native review
cheatsheet-medical-3d = 3D 和重建
cheatsheet-medical-analysis = 分割、融合和 PET
# machine-drafted, needs native review
cheatsheet-medical-studies = 检查与共享
cheatsheet-palette-only-title = 在命令面板中（{ $key }）
cheatsheet-palette-only-title-nokey = 在命令面板中
cheatsheet-palette-only-tag = 面板
cheatsheet-palette-only-line = { $action }：无快捷键；在命令面板中（{ $key }）
cheatsheet-palette-only-line-nokey = { $action }：无快捷键；在命令面板中
# 快捷键速查表上的 Esc（命令面板中对它的长描述放不下）。
cheatsheet-back = 关闭或返回

## Hardware video encoding (machine-drafted, needs native review)
hw-report-path-video-encode = 视频导出
hw-report-engine-hw-encoder = 硬件编码器

## Path timing units (machine-drafted, needs native review)
hw-report-task-thumbnail = { $task }（每张 { $size } 缩略图）
hw-report-task-frame = { $task }（每帧 { $size }）

## Hardware report, software-only formats (machine-drafted, needs native review)
hw-report-codec-software = 仅软件

about-natural-earth = 地图数据来自 Natural Earth (naturalearthdata.com)，公有领域。
app-info-show-on-map = 在地图上显示
