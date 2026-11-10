# machine-drafted, needs native review
# Specular — Develop (non-destructive photo editing). 简体中文.

# Shown once when an edit opens and some of its modules were made with a version that no
# longer renders: they were moved to the nearest version that does. $count: modules moved.
# machine-drafted, needs native review
toast-develop-edit-migrated = { $count ->
   *[other] 此编辑中有 { $count } 个模块已更新到较新版本，效果可能略有不同。
}
# Shown when an edit holds modules this version of Specular cannot render (made with a newer
# Specular). They are kept and saved unchanged, but skipped. $count: modules skipped.
# machine-drafted, needs native review
toast-develop-edit-newer = { $count ->
   *[other] 此编辑中有 { $count } 个模块需要较新版本的 Specular。这些模块会保留，但不会显示。
}

## Develop's controls (sliders, curve, colour wheels). Numbers come formatted in the UI
## language's digits.

# machine-drafted, needs native review
# A slider's value in exposure values. $value: "+0.70".
develop-value-ev = { $value } EV
# machine-drafted, needs native review
# machine-drafted, needs native review
# An angle. $value: "2.5".
develop-value-degrees = { $value }°
# machine-drafted, needs native review
# A colour temperature. $value: "5,000".
develop-value-kelvin = { $value } K
# machine-drafted, needs native review
# The curve editor's selected point. $index, $count: point numbers; $input, $output: "25 %".
develop-curve-point = 第 { $index } 个点，共 { $count } 个：输入 { $input }，输出 { $output }
# machine-drafted, needs native review
# The curve editor's keys, read by screen readers.
develop-curve-keys = 方向键移动该点。Page Up 和 Page Down 选择其他点。Insert 添加点，Delete 删除点。
# machine-drafted, needs native review
# A colour wheel with no colour.
develop-wheel-neutral = 中性
# machine-drafted, needs native review
# A colour wheel's colour. $hue: degrees; $strength: "20 %".
develop-wheel-value = 色相 { $hue }°，强度 { $strength }
# machine-drafted, needs native review
# A colour wheel's keys, read by screen readers.
develop-wheel-keys = 方向键移动颜色。Delete 将其复位。

## Develop's panel.

# machine-drafted, needs native review
develop-title = 修改照片
# machine-drafted, needs native review
develop-undo = 撤销
# machine-drafted, needs native review
develop-redo = 重做
# machine-drafted, needs native review
develop-before-after = 修改前和修改后
# machine-drafted, needs native review
develop-close = 退出修改照片
# machine-drafted, needs native review
develop-history = 历史记录
# machine-drafted, needs native review
# A card's on/off switch. $module: the module's name.
develop-module-on = { $module }已开启
develop-card-menu = { $module }选项
develop-card-changed = 已更改
# machine-drafted, needs native review
# The speed-edit readout over the image. $control: "Exposure"; $value: "+0.70 EV".
develop-speed-hud = { $control }：{ $value }
# machine-drafted, needs native review
develop-filmstrip = 胶片带

## Develop's module cards and controls.

# machine-drafted, needs native review
develop-module-exposure = 曝光
develop-module-light = 光线
develop-look-profile = 配置文件
develop-look-standard = 标准
develop-look-film = 胶片
develop-look-natural = 自然
develop-module-grading = 颜色分级
develop-add-tool = 添加工具
# machine-drafted, needs native review
develop-module-white-balance = 白平衡
# machine-drafted, needs native review
develop-module-tone = 影调
# machine-drafted, needs native review
develop-module-contrast = 对比度和范围
# machine-drafted, needs native review
develop-module-colour = 颜色
# machine-drafted, needs native review
develop-module-curve = 曲线
# machine-drafted, needs native review
develop-module-crop = 裁剪和拉直
# machine-drafted, needs native review
# A module this version has no card for.
develop-module-other = 其他模块
# machine-drafted, needs native review
develop-exposure = 曝光度
# machine-drafted, needs native review
develop-black-level = 黑电平
# machine-drafted, needs native review
develop-temperature = 色温
# machine-drafted, needs native review
develop-tint = 色调
# machine-drafted, needs native review
develop-highlights = 高光
# machine-drafted, needs native review
develop-shadows = 阴影
# machine-drafted, needs native review
develop-contrast = 对比度
# machine-drafted, needs native review
develop-whites = 白色色阶
# machine-drafted, needs native review
develop-blacks = 黑色色阶
# machine-drafted, needs native review
# The view transform: how scene brightness becomes screen brightness.
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# machine-drafted, needs native review
# HDR card: how far above SDR white the HDR picture may go, and which picture to show.
develop-hdr = HDR
# machine-drafted, needs native review
develop-hdr-limit = HDR 上限
# machine-drafted, needs native review
develop-hdr-preview = 显示
# machine-drafted, needs native review
develop-hdr-preview-hdr = HDR
# machine-drafted, needs native review
develop-hdr-preview-sdr = SDR
# machine-drafted, needs native review
develop-vibrance = 自然饱和度
# machine-drafted, needs native review
develop-saturation = 饱和度
# machine-drafted, needs native review
develop-colour-wheels = 色轮
# machine-drafted, needs native review
develop-curve = 色调曲线
# machine-drafted, needs native review
develop-module-local-contrast = 局部对比度
# machine-drafted, needs native review
# A switch: exposure set from the photo's own brightness.
develop-auto-exposure = 自动曝光
# machine-drafted, needs native review
# How far the tone equalizer's highlights and shadows spread across an area.
develop-smoothing = 平滑
# machine-drafted, needs native review
# Local contrast: fine detail, and larger-scale contrast.
develop-texture = 纹理
# machine-drafted, needs native review
develop-clarity = 清晰度
# machine-drafted, needs native review
# Levels on the curve card: the input that becomes black, the middle and white.
develop-levels-black = 黑点
# machine-drafted, needs native review
develop-levels-mid = 中间点
# machine-drafted, needs native review
develop-levels-white = 白点
# machine-drafted, needs native review
develop-module-detail = 细节
# machine-drafted, needs native review
develop-module-denoise = 降噪
# machine-drafted, needs native review
develop-module-sharpen = 锐化
# machine-drafted, needs native review
develop-noise-luminance = 明亮度噪点
# machine-drafted, needs native review
develop-noise-detail = 噪点细节
# machine-drafted, needs native review
develop-noise-colour = 颜色噪点
# machine-drafted, needs native review
develop-sharpen-amount = 锐化数量
# machine-drafted, needs native review
develop-sharpen-radius = 锐化半径
# machine-drafted, needs native review
develop-sharpen-detail = 锐化细节
# machine-drafted, needs native review
develop-sharpen-masking = 蒙版
# machine-drafted, needs native review
develop-capture-amount = 捕获锐化
# machine-drafted, needs native review
develop-capture-radius = 捕获半径
# machine-drafted, needs native review
develop-straighten = 拉直
# machine-drafted, needs native review
# History rows of the crop tool.
develop-crop = 裁剪
# machine-drafted, needs native review
develop-crop-turn = 旋转
# machine-drafted, needs native review
develop-crop-flip = 翻转
# machine-drafted, needs native review
# The lens card: the camera's own corrections (its switch) and manual ones.
develop-module-lens = 镜头校正
develop-module-optics = 光学
# machine-drafted, needs native review
develop-lens-distortion = 畸变
# machine-drafted, needs native review
develop-lens-vignette = 暗角
# machine-drafted, needs native review
# Colour fringes at the edges of the frame (lateral chromatic aberration), per colour.
develop-lens-tca-red = 红色边缘
# machine-drafted, needs native review
develop-lens-tca-blue = 蓝色边缘
# machine-drafted, needs native review
# The Lensfun lens profile for raws without corrections of their own: a picker and a note.
develop-lensfun-profile = 镜头配置文件
# machine-drafted, needs native review
# The picker's first option: the profile matched to the file's lens name.
develop-lensfun-auto = 自动
# machine-drafted, needs native review
develop-lensfun-off = 无
# machine-drafted, needs native review
# $lens: "Canon EF 24-70mm f/2.8L II USM". Lensfun is a name; keep it.
develop-lensfun-used = Lensfun: { $lens }
# machine-drafted, needs native review
develop-lensfun-none-chosen = 无镜头配置文件。
# machine-drafted, needs native review
# $lens: the lens name the file gives.
develop-lensfun-no-match = 没有适用于 { $lens } 的 Lensfun 配置文件。
# machine-drafted, needs native review
develop-lensfun-no-profile = 没有适用于此镜头的 Lensfun 配置文件。
# machine-drafted, needs native review
develop-lensfun-no-name = 文件未注明镜头。请在上方选择。
# machine-drafted, needs native review
# $count: how many lenses match equally well (2 or more).
develop-lensfun-ambiguous = 有 { $count } 个镜头同样匹配。请在上方选择一个。

## The crop tool over the photo (key R). Said to screen readers, or shown over the photo.

# machine-drafted, needs native review
develop-crop-open = 裁剪。拖动边缘，或按住 Ctrl 沿应当水平的线拖动。Enter 保留，Esc 返回。
# machine-drafted, needs native review
develop-crop-kept = 已保留裁剪
# machine-drafted, needs native review
develop-crop-cancelled = 已撤销裁剪
# machine-drafted, needs native review
develop-crop-reset = 已重置裁剪和拉直
# machine-drafted, needs native review
# $angle: "-1.5".
develop-crop-straightened = 已拉直 { $angle }°
# machine-drafted, needs native review
# $aspect: "3:2", or one of the two names below.
develop-crop-aspect = 长宽比 { $aspect }
# machine-drafted, needs native review
develop-crop-aspect-free = 自由
# machine-drafted, needs native review
develop-crop-aspect-original = 原始
# machine-drafted, needs native review
develop-crop-turned-right = 已向右旋转
# machine-drafted, needs native review
develop-crop-turned-left = 已向左旋转
# machine-drafted, needs native review
develop-crop-flipped-horizontal = 已左右翻转
# machine-drafted, needs native review
develop-crop-flipped-vertical = 已上下翻转
# machine-drafted, needs native review
# The label over the photo. $aspect: as above; $guide: one of the guide names below.
develop-crop-label = 长宽比 { $aspect } · 参考线：{ $guide }
# machine-drafted, needs native review
develop-crop-keys = A 长宽比 · X 互换 · O 参考线 · [ ] 旋转 · H V 翻转 · Enter 保留 · Esc 返回
# machine-drafted, needs native review
develop-crop-guide-thirds = 三分法
# machine-drafted, needs native review
develop-crop-guide-golden = 黄金分割
# machine-drafted, needs native review
develop-crop-guide-diagonal = 对角线
# machine-drafted, needs native review
develop-crop-guide-grid = 网格

## Perspective: keystone and upright (its own card; guides are drawn in the crop tool).

# machine-drafted, needs native review
develop-module-perspective = 透视
# machine-drafted, needs native review
# Upright: how the camera's tilt is found.
develop-persp-upright = 扶正
# machine-drafted, needs native review
develop-persp-mode-off = 关
# machine-drafted, needs native review
develop-persp-mode-auto = 自动
# machine-drafted, needs native review
develop-persp-mode-level = 水平
# machine-drafted, needs native review
develop-persp-mode-vertical = 垂直
# machine-drafted, needs native review
develop-persp-mode-full = 完全
# machine-drafted, needs native review
develop-persp-mode-guided = 引导
# machine-drafted, needs native review
develop-persp-vertical = 垂直
# machine-drafted, needs native review
develop-persp-horizontal = 水平
# machine-drafted, needs native review
develop-persp-rotate = 旋转
# machine-drafted, needs native review
develop-persp-aspect = 长宽比
# machine-drafted, needs native review
develop-persp-scale = 缩放
# machine-drafted, needs native review
develop-persp-offset-x = 左右
# machine-drafted, needs native review
develop-persp-offset-y = 上下
# machine-drafted, needs native review
# Zoom so no empty edges show.
develop-persp-auto-scale = 避免空白边缘
# machine-drafted, needs native review
develop-persp-guides = 参考线
# machine-drafted, needs native review
develop-persp-focal = 焦距
# machine-drafted, needs native review
develop-persp-draw-guides = 绘制参考线
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-draw-guides-n = 绘制参考线（{ $n }/{ $max }）
# machine-drafted, needs native review
develop-persp-reset = 重置透视
# machine-drafted, needs native review
# $mode: one of the mode names above.
develop-persp-mode-set = 扶正：{ $mode }
# machine-drafted, needs native review
develop-persp-analysing = 正在查找线条…
# machine-drafted, needs native review
# $n: the guides drawn.
develop-persp-reset-confirm = 重置透视？你绘制的 { $n } 条参考线将被移除。撤销可恢复。
# machine-drafted, needs native review
develop-persp-reset-ok = 重置
# machine-drafted, needs native review
develop-persp-reset-done = 已重置透视
# machine-drafted, needs native review
# $n: the guide's number.
develop-persp-guide-remove-confirm = 移除参考线 { $n }？撤销可恢复。
# machine-drafted, needs native review
develop-persp-guide-remove-ok = 移除
# machine-drafted, needs native review
develop-persp-guide-removed = 已移除参考线
# machine-drafted, needs native review
# $vertical, $horizontal: the slider values.
develop-persp-keystone-now = 垂直 { $vertical }，水平 { $horizontal }
# machine-drafted, needs native review
# $error: what went wrong.
develop-persp-failed = 扶正失败：{ $error }
# machine-drafted, needs native review
# $mode: a mode name; the angles in degrees ("2.5").
develop-persp-solved = { $mode }：俯仰 { $vertical }°，偏转 { $horizontal }°，旋转 { $rotate }°
# machine-drafted, needs native review
# $mode: a mode name.
develop-perspective-few-lines = { $mode } 需要至少两条可区分的直线。请换一种模式，或绘制参考线。
# machine-drafted, needs native review
# $n: 4.
develop-persp-guides-full = 最多 { $n } 条参考线。拖动端点可移动。
# machine-drafted, needs native review
develop-persp-guides-on = 参考线。沿应当竖直或水平的线拖动。
# machine-drafted, needs native review
develop-persp-guides-off = 裁剪
# machine-drafted, needs native review
develop-persp-guide-moved = 已移动参考线
# machine-drafted, needs native review
# $n: how many guides now.
develop-persp-guide-added = 已添加参考线 { $n }
# machine-drafted, needs native review
develop-persp-guides-keys = 拖动添加 · 拖动端点移动 · Delete 删除 · Shift+R 裁剪
# machine-drafted, needs native review
# $n: guides drawn; $max: 4.
develop-persp-guides-label = 参考线：{ $n }/{ $max }
# machine-drafted, needs native review
# The three colour wheels.
develop-wheel-shadows = 阴影
# machine-drafted, needs native review
develop-wheel-midtones = 中间调
# machine-drafted, needs native review
develop-wheel-highlights = 高光
# machine-drafted, needs native review
# The luminance slider under each colour wheel (read by screen readers).
develop-luminance-shadows = 阴影明亮度
# machine-drafted, needs native review
develop-luminance-midtones = 中间调明亮度
# machine-drafted, needs native review
develop-luminance-highlights = 高光明亮度
# machine-drafted, needs native review
# The fourth colour wheel: the whole picture.
develop-wheel-global = 全局
# machine-drafted, needs native review
develop-luminance-global = 全局明亮度
# machine-drafted, needs native review
# Colour card: colourfulness changed evenly, and the lightness of colourful areas.
develop-chroma = 色彩强度
# machine-drafted, needs native review
develop-brilliance = 色彩亮度

## Colour mixer: hue, saturation and brightness per colour.

# machine-drafted, needs native review
develop-module-colour-mixer = 混色器
# machine-drafted, needs native review
develop-colour-red = 红色
# machine-drafted, needs native review
develop-colour-orange = 橙色
# machine-drafted, needs native review
develop-colour-yellow = 黄色
# machine-drafted, needs native review
develop-colour-green = 绿色
# machine-drafted, needs native review
develop-colour-aqua = 浅绿色
# machine-drafted, needs native review
develop-colour-blue = 蓝色
# machine-drafted, needs native review
develop-colour-purple = 紫色
# machine-drafted, needs native review
develop-colour-magenta = 洋红
# machine-drafted, needs native review
# $colour: one of the colour names above.
develop-mixer-hue = { $colour }色相
# machine-drafted, needs native review
develop-mixer-saturation = { $colour }饱和度
# machine-drafted, needs native review
develop-mixer-brightness = { $colour }明亮度

## Point colour: pick a colour in the photo, then shift it and its neighbours.

# machine-drafted, needs native review
develop-module-point-colour = 点颜色
# machine-drafted, needs native review
develop-point-pick = 选取颜色
# machine-drafted, needs native review
# On the pick button once a colour is picked.
develop-point-pick-again = 选取其他颜色
# machine-drafted, needs native review
# Said and shown while picking.
develop-point-picking = 点击要更改的颜色。按 Esc 取消。
# machine-drafted, needs native review
develop-point-picked = 已选取颜色
# machine-drafted, needs native review
develop-point-cancelled = 已取消选取
# machine-drafted, needs native review
# How far from the picked colour changes reach: overall, then by hue, saturation and lightness.
develop-point-range = 范围
# machine-drafted, needs native review
develop-point-hue-range = 色相范围
# machine-drafted, needs native review
develop-point-saturation-range = 饱和度范围
# machine-drafted, needs native review
develop-point-lightness-range = 明度范围
# machine-drafted, needs native review
develop-point-hue = 色相偏移
# machine-drafted, needs native review
develop-point-saturation = 饱和度偏移
# machine-drafted, needs native review
develop-point-lightness = 明度偏移

## Black and white.

# machine-drafted, needs native review
develop-module-black-white = 黑白
# machine-drafted, needs native review
# $colour: a colour name; how light that colour turns in grey.
develop-bw-mix = { $colour }明亮度

## LUT: a colour look-up table file.

# machine-drafted, needs native review
develop-module-lut = LUT
# machine-drafted, needs native review
develop-lut-choose = 选择 LUT 文件…
# machine-drafted, needs native review
# The file chosen. $name: its file name.
develop-lut-file = 文件：{ $name }
# machine-drafted, needs native review
# The file can't be used. $name: its file name.
develop-lut-unreadable = 无法读取 { $name }
# machine-drafted, needs native review
# The file dialog.
develop-lut-dialog = 选择 LUT 文件
# machine-drafted, needs native review
develop-lut-kind = LUT 文件
# machine-drafted, needs native review
# The encoding the LUT was made for.
develop-lut-space = 适用色彩空间
# machine-drafted, needs native review
develop-lut-space-linear = 线性 (Rec.2020)
# machine-drafted, needs native review
develop-lut-space-acescg = ACES 线性 (ACEScg)
# machine-drafted, needs native review
develop-lut-space-acescct = ACES 对数 (ACEScct)
# machine-drafted, needs native review
develop-lut-space-srgb = 照片 (sRGB)
# machine-drafted, needs native review
develop-lut-space-rec709 = 视频 (Rec.709)
# machine-drafted, needs native review
develop-lut-interpolation = 方法
# machine-drafted, needs native review
develop-lut-tetrahedral = 精确
# machine-drafted, needs native review
develop-lut-trilinear = 简单
# machine-drafted, needs native review
develop-lut-strength = 强度

## Develop's history and status.

# machine-drafted, needs native review
develop-history-original = 原图
# machine-drafted, needs native review
# A module switched on or off. $module: its name.
develop-history-on = { $module }已开启
# machine-drafted, needs native review
develop-history-off = { $module }已关闭
# machine-drafted, needs native review
# Said to screen readers after a click on a history row. $step: the row's name.
develop-at-original = 已回到原图
# machine-drafted, needs native review
develop-at-step = 已回到：{ $step }
# machine-drafted, needs native review
develop-undone = 已撤销：{ $step }
# machine-drafted, needs native review
develop-redone = 已重做：{ $step }
# machine-drafted, needs native review
develop-entered = 修改照片。编辑永远不会更改文件。
# machine-drafted, needs native review
develop-left = 已退出修改照片
# machine-drafted, needs native review
develop-split-on = 修改前和修改后并排显示
# machine-drafted, needs native review
develop-split-off = 仅编辑
# machine-drafted, needs native review
develop-showing-before = 正在显示原图
# machine-drafted, needs native review
develop-showing-after = 正在显示编辑
# machine-drafted, needs native review
# Labels over the image.
develop-before = 修改前
# machine-drafted, needs native review
develop-after = 修改后
# machine-drafted, needs native review
develop-working = 正在处理…
# machine-drafted, needs native review
develop-preview-failed = 无法显示此文件的编辑

## Develop's commands (palette, cheat sheet, Settings → Keys) and why they are unavailable.

# machine-drafted, needs native review
keys-mode-develop = 修改照片
# machine-drafted, needs native review
# The cheat sheet's heading over the speed-edit keys.
cheatsheet-develop-speed = 快速编辑：按住后滚动
# machine-drafted, needs native review
palette-develop = 修改照片
# machine-drafted, needs native review
palette-develop-help = 编辑此照片而不更改文件。再次运行可退出。
# machine-drafted, needs native review
palette-develop-before-after = 修改前和修改后
# machine-drafted, needs native review
palette-develop-before-after-help = 显示原图，或重新显示编辑。
# machine-drafted, needs native review
palette-develop-split = 拆分修改前和修改后
# machine-drafted, needs native review
palette-develop-split-help = 并排显示原图和编辑。
# machine-drafted, needs native review
palette-develop-panel = 转到修改照片面板
# machine-drafted, needs native review
palette-develop-panel-help = 将键盘焦点移到面板。按 Esc 返回。
# machine-drafted, needs native review
palette-develop-speed-exposure = 快速编辑曝光度
# machine-drafted, needs native review
palette-develop-speed-exposure-help = 按住后滚动或按方向键以更改曝光度。
# machine-drafted, needs native review
palette-develop-speed-contrast = 快速编辑对比度
# machine-drafted, needs native review
palette-develop-speed-contrast-help = 按住后滚动或按方向键以更改对比度。
# machine-drafted, needs native review
palette-develop-speed-highlights = 快速编辑高光
# machine-drafted, needs native review
palette-develop-speed-highlights-help = 按住后滚动或按方向键以更改高光。
# machine-drafted, needs native review
palette-develop-speed-shadows = 快速编辑阴影
# machine-drafted, needs native review
palette-develop-speed-shadows-help = 按住后滚动或按方向键以更改阴影。
# machine-drafted, needs native review
palette-develop-speed-whites = 快速编辑白色色阶
# machine-drafted, needs native review
palette-develop-speed-whites-help = 按住后滚动或按方向键以更改白色色阶。
# machine-drafted, needs native review
palette-develop-speed-blacks = 快速编辑黑色色阶
# machine-drafted, needs native review
palette-develop-speed-blacks-help = 按住后滚动或按方向键以更改黑色色阶。
# machine-drafted, needs native review
palette-develop-speed-temperature = 快速编辑色温
# machine-drafted, needs native review
palette-develop-speed-temperature-help = 按住后滚动或按方向键以更改白平衡的色温。
# machine-drafted, needs native review
palette-develop-speed-vibrance = 快速编辑自然饱和度
# machine-drafted, needs native review
palette-develop-speed-vibrance-help = 按住后滚动或按方向键以更改自然饱和度。
# machine-drafted, needs native review
palette-develop-crop = 裁剪和拉直
# machine-drafted, needs native review
palette-develop-crop-help = 裁剪、拉直、旋转或翻转照片。Enter 保留，Esc 返回。
# machine-drafted, needs native review
disabled-not-photo = 仅适用于静态照片
# machine-drafted, needs native review
disabled-develop-off = 医学和科学使用场景中已关闭修改照片
# machine-drafted, needs native review
develop-module-corrections = 校正
# machine-drafted, needs native review
develop-module-raw = RAW
# machine-drafted, needs native review
develop-off = 关
# machine-drafted, needs native review
develop-on = 开
# machine-drafted, needs native review
develop-highlight-method = 高光溢出
# machine-drafted, needs native review
develop-highlight-opposed = 重建颜色
# machine-drafted, needs native review
develop-highlight-rebuild = 重建大面积
# machine-drafted, needs native review
develop-highlight-clip = 裁切为白色
# machine-drafted, needs native review
develop-highlight-threshold = 溢出阈值
# machine-drafted, needs native review
develop-module-hot-pixels = 热像素
# machine-drafted, needs native review
develop-hot-threshold = 热像素阈值
# machine-drafted, needs native review
develop-module-ca = 色差
# machine-drafted, needs native review
develop-ca-mode = 色边
# machine-drafted, needs native review
develop-ca-auto = 自动
# machine-drafted, needs native review
develop-ca-manual = 手动
# machine-drafted, needs native review
develop-ca-red = 红色偏移
# machine-drafted, needs native review
develop-ca-blue = 蓝色偏移
# machine-drafted, needs native review
develop-module-defringe = 去紫边
# machine-drafted, needs native review
develop-defringe = 去紫边
# machine-drafted, needs native review
develop-defringe-purple = 紫色色相
# machine-drafted, needs native review
develop-defringe-green = 绿色色相
# machine-drafted, needs native review
develop-defringe-range = 色相范围
# machine-drafted, needs native review
develop-defringe-edge = 边缘对比度
# machine-drafted, needs native review
develop-module-flat-field = 校准帧
# machine-drafted, needs native review
develop-calibration = 校准
# machine-drafted, needs native review
develop-calibration-auto = 来自文件夹
# machine-drafted, needs native review
develop-calibration-chosen = 所选文件
# machine-drafted, needs native review
develop-flat-choose = 选择平场帧…
# machine-drafted, needs native review
develop-dark-choose = 选择暗场帧…
# machine-drafted, needs native review
develop-flat-file = 平场：{ $name }
# machine-drafted, needs native review
develop-dark-file = 暗场：{ $name }
# machine-drafted, needs native review
develop-flat-use = 平场
# machine-drafted, needs native review
develop-flat-use-full = 暗角和灰尘
# machine-drafted, needs native review
develop-flat-use-vignetting = 仅暗角
# machine-drafted, needs native review
develop-flat-dialog = 选择平场帧
# machine-drafted, needs native review
develop-dark-dialog = 选择暗场帧
# machine-drafted, needs native review
develop-raw-kind = RAW 文件
# machine-drafted, needs native review
develop-flat-frame = 平场帧
# machine-drafted, needs native review
develop-dark-frame = 暗场帧

# machine-drafted, needs native review
# machine-drafted, needs native review
develop-mask-add = 添加蒙版
# machine-drafted, needs native review
develop-mask-kind-ellipse = 椭圆
# machine-drafted, needs native review
develop-mask-kind-rectangle = 矩形
# machine-drafted, needs native review
develop-mask-kind-polygon = 多边形
# machine-drafted, needs native review
develop-mask-kind-linear = 线性渐变
# machine-drafted, needs native review
develop-mask-kind-radial = 径向渐变
# machine-drafted, needs native review
develop-mask-kind-brush = 画笔
# machine-drafted, needs native review
develop-mask-kind-luminance = 亮度范围
# machine-drafted, needs native review
develop-mask-kind-hue = 色相范围
# machine-drafted, needs native review
develop-mask-kind-chroma = 饱和度范围
# machine-drafted, needs native review
develop-mask-kind-colour = 颜色范围
# machine-drafted, needs native review
develop-mask-kind-other = 其他蒙版
# machine-drafted, needs native review
develop-mask-row = { $kind } { $n }
# machine-drafted, needs native review
develop-mask-mode = 混合
# machine-drafted, needs native review
develop-blend-normal = 正常
# machine-drafted, needs native review
develop-blend-multiply = 正片叠底
# machine-drafted, needs native review
develop-blend-screen = 滤色
# machine-drafted, needs native review
develop-blend-overlay = 叠加
# machine-drafted, needs native review
develop-blend-soft-light = 柔光
# machine-drafted, needs native review
develop-blend-lightness = 仅明度
# machine-drafted, needs native review
develop-blend-colour = 仅颜色
# machine-drafted, needs native review
develop-mask-amount = 蒙版强度
# machine-drafted, needs native review
develop-mask-invert = 反转蒙版
# machine-drafted, needs native review
develop-mask-off = 关
# machine-drafted, needs native review
develop-mask-on = 开
# machine-drafted, needs native review
develop-mask-refine = 优化边缘
# machine-drafted, needs native review
develop-mask-show = 显示蒙版
# machine-drafted, needs native review
develop-mask-show-red = 红色
# machine-drafted, needs native review
develop-mask-show-grey = 灰色
# machine-drafted, needs native review
develop-mask-remove = 移除蒙版
# machine-drafted, needs native review
develop-mask-combine-add = 添加
# machine-drafted, needs native review
develop-mask-combine-subtract = 减去
# machine-drafted, needs native review
develop-mask-combine-intersect = 相交
# machine-drafted, needs native review
develop-mask-row-opacity = 不透明度
# machine-drafted, needs native review
develop-mask-row-invert = 反转
# machine-drafted, needs native review
develop-mask-feather = 羽化
# machine-drafted, needs native review
develop-mask-roundness = 圆角
# machine-drafted, needs native review
develop-mask-brush-size = 画笔大小
# machine-drafted, needs native review
develop-mask-brush-feather = 画笔羽化
# machine-drafted, needs native review
develop-mask-brush-flow = 流量
# machine-drafted, needs native review
develop-mask-brush-mode = 画笔
# machine-drafted, needs native review
develop-mask-brush-paint = 绘制
# machine-drafted, needs native review
develop-mask-brush-erase = 擦除
# machine-drafted, needs native review
develop-mask-range-from = 起点
# machine-drafted, needs native review
develop-mask-range-full-from = 完全起点
# machine-drafted, needs native review
develop-mask-range-full-to = 完全终点
# machine-drafted, needs native review
develop-mask-range-to = 终点
# machine-drafted, needs native review
develop-mask-range-test = 检测
# machine-drafted, needs native review
develop-mask-range-input = 输入
# machine-drafted, needs native review
develop-mask-range-output = 输出
# machine-drafted, needs native review
develop-mask-colour-pick = 拾取颜色
# machine-drafted, needs native review
develop-mask-colour-pick-more = 再拾取一种颜色
# machine-drafted, needs native review
develop-mask-colour-range = 范围
# machine-drafted, needs native review
develop-mask-colour-falloff = 衰减
# machine-drafted, needs native review
develop-mask-edit = 在图像上编辑
# machine-drafted, needs native review
develop-mask-row-remove = 移除 { $name }
# machine-drafted, needs native review
develop-mask-remove-confirm = 移除 { $module } 的蒙版？其形状会被删除。磁盘上的照片保留。
# machine-drafted, needs native review
develop-mask-row-remove-confirm = 从 { $module } 移除 { $name }？其他蒙版和磁盘上的照片保留。
# machine-drafted, needs native review
develop-mask-remove-ok = 移除
# machine-drafted, needs native review
develop-mask-removed = 已移除蒙版。
# machine-drafted, needs native review
develop-mask-added = 已添加 { $name }。
# machine-drafted, needs native review
develop-mask-full = 此蒙版已达到可容纳的部分上限。
# machine-drafted, needs native review
develop-mask-editing = 正在编辑 { $name }。拖动控制点，或按 Tab 和方向键。
# machine-drafted, needs native review
develop-mask-painting = 正在绘制 { $name }。在图像上拖动。
# machine-drafted, needs native review
develop-mask-picking = 点击图像拾取颜色。
# machine-drafted, needs native review
develop-mask-picked = 已拾取颜色。
# machine-drafted, needs native review
develop-mask-done = 蒙版编辑完成。
# machine-drafted, needs native review
develop-mask-handles = 蒙版控制点
# machine-drafted, needs native review
develop-handle-centre = 中心
# machine-drafted, needs native review
develop-handle-width = 宽度
# machine-drafted, needs native review
develop-handle-height = 高度
# machine-drafted, needs native review
develop-handle-rotate = 旋转
# machine-drafted, needs native review
develop-handle-feather = 羽化
# machine-drafted, needs native review
develop-handle-inner = 内边缘
# machine-drafted, needs native review
develop-handle-start = 起点
# machine-drafted, needs native review
develop-handle-end = 终点
# machine-drafted, needs native review
develop-handle-point = 点 { $n }
# machine-drafted, needs native review
develop-instance = 图层
# machine-drafted, needs native review
develop-instance-n = { $n }
# machine-drafted, needs native review
develop-instance-chosen = 图层 { $n }。
# machine-drafted, needs native review
develop-instance-new = 新建图层
# machine-drafted, needs native review
develop-instance-duplicate = 复制图层
# machine-drafted, needs native review
develop-instance-delete = 删除图层
# machine-drafted, needs native review
develop-instance-earlier = 前移
# machine-drafted, needs native review
develop-instance-later = 后移
# machine-drafted, needs native review
develop-instance-delete-confirm = 删除 { $module } 的图层 { $n }？其设置和蒙版会被删除。磁盘上的照片保留。
# machine-drafted, needs native review
develop-instance-delete-ok = 删除
# machine-drafted, needs native review
develop-instance-deleted = 已删除图层。
# machine-drafted, needs native review
develop-history-mask = 蒙版：{ $module }
# machine-drafted, needs native review
develop-history-mask-shapes = 蒙版形状：{ $module }
# machine-drafted, needs native review
develop-history-instance-new = 新建：{ $module }
# machine-drafted, needs native review
develop-history-instance-delete = 删除：{ $module }
# machine-drafted, needs native review
develop-history-instance-move = 顺序：{ $module }
# Other tools' edits and sidecar conflicts.
# machine-drafted, needs native review
develop-others-title = 其他软件
# machine-drafted, needs native review
develop-darktable-edited = 已在 darktable 中编辑：
# machine-drafted, needs native review
develop-darktable-open = 在 darktable 中打开
# machine-drafted, needs native review
develop-darktable-opening = 正在 darktable 中打开
# machine-drafted, needs native review
develop-darktable-failed = darktable 未能启动：{ $reason }
# machine-drafted, needs native review
develop-lightroom-import = 从 Lightroom 导入
# machine-drafted, needs native review
develop-lightroom-imported = { $count ->
   *[other] 已导入 { $count } 项 Lightroom 设置。结果为近似。
}
# machine-drafted, needs native review
develop-lightroom-nothing = 没有可导入的内容：这些设置在这里不改变任何东西。
# machine-drafted, needs native review
develop-history-lightroom = 从 Lightroom 导入（近似）
# machine-drafted, needs native review
develop-conflict-title = { $file } 的编辑在 Specular 之外被更改
# machine-drafted, needs native review
develop-conflict-file = 附属文件：保存于 { $time }
# machine-drafted, needs native review
develop-conflict-library = 图库：保存于 { $time }
# machine-drafted, needs native review
develop-conflict-writer = 附属文件由 { $writer } 写入。
# machine-drafted, needs native review
develop-conflict-detail = 保留一个。另一个会被替换。
# machine-drafted, needs native review
develop-conflict-use-file = 使用文件中的
# machine-drafted, needs native review
develop-conflict-keep-library = 保留图库中的
# machine-drafted, needs native review
develop-conflict-used-file = 正在使用附属文件中的编辑
# machine-drafted, needs native review
develop-conflict-kept-library = 保留图库中的编辑
# machine-drafted, needs native review
toast-develop-edit-newer-process = 此编辑由更新版本的 Specular 制作。在这里可能看起来不同。
# machine-drafted, needs native review
info-darktable-edited = 已在 darktable 中编辑
# machine-drafted, needs native review
info-darktable-no-modules = 没有开启的模块
# machine-drafted, needs native review
info-darktable-group = 文件
# machine-drafted, needs native review
toast-develop-edit-partial = 此编辑由更新版本的 Specular 制作。此版本无法显示的部分已省略。此处的更改不会保存。
# machine-drafted, needs native review
toast-develop-sidecar-unreadable = 无法读取照片旁的编辑文件。它保持原样。此处的更改不会保存。
# machine-drafted, needs native review
develop-demosaic = RAW 细节
develop-demosaic-directional = 标准
develop-demosaic-dual = 平滑平坦区域
develop-demosaic-fast = 快速
develop-dual-threshold = 平坦区域对比度
