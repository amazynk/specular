# machine-drafted, needs native review
# 检查工具：裁剪警告、峰值对焦、直方图、像素检查器。

## 命令面板
palette-clipping = 显示或隐藏裁剪警告
palette-focus-peaking = 显示或隐藏峰值对焦
palette-histogram = 显示或隐藏直方图
palette-pixel-inspector = 显示或隐藏像素检查器
# machine-drafted, needs native review
palette-crosshair = 键盘十字线
palette-announce-cursor = 朗读光标下的值

## 直方图
histogram-title = 直方图
histogram-clipped = 裁剪：高光 { $high }，阴影 { $low }
histogram-empty = 没有可见的图像
histogram-a11y = 可见图像的直方图：平均亮度 { $mean }，高光裁剪 { $high }，阴影裁剪 { $low }

## 像素检查器
inspector-title = 像素检查器
inspector-position = 位置
inspector-source-rgb = 源 RGB
inspector-source-rgba = 源 RGBA
inspector-source-ycc = 源 Y′CbCr
inspector-source-gray = 源 灰度
inspector-source-gray16 = 源 灰度（16 位）
inspector-source-gray-alpha16 = 源 灰度 + Alpha（16 位）
inspector-source-rgb16 = 源 RGB（16 位）
inspector-source-rgba16 = 源 RGBA（16 位）
inspector-source-pending = 源
inspector-srgb = sRGB
inspector-hex = 十六进制
inspector-linear = 线性
inspector-approximate = 来自缩小的预览；放大以获得精确值
inspector-outside = 将指针移到图像上以检查像素
inspector-a11y = 位于 { $x }, { $y } 的像素：sRGB { $value }

## 设置
settings-section-inspection = 检查
settings-inspection-clip-highlight = 高光裁剪阈值
settings-inspection-clip-highlight-help = 任一通道达到或超过此 8 位值的像素标记为高光裁剪。
settings-inspection-clip-shadow = 阴影裁剪阈值
settings-inspection-clip-shadow-help = 任一通道等于或低于此 8 位值的像素标记为阴影裁剪。
settings-inspection-highlight-color = 高光警告颜色
settings-inspection-highlight-color-help = 标记高光裁剪的颜色。
settings-inspection-shadow-color = 阴影警告颜色
settings-inspection-shadow-color-help = 标记阴影裁剪的颜色。
settings-inspection-peaking-sensitivity = 峰值对焦灵敏度
settings-inspection-peaking-sensitivity-help = 数值越高，较柔和的边缘也会被标记为合焦。
settings-inspection-peaking-color = 峰值对焦颜色
settings-inspection-peaking-color-help = 标记锐利边缘的颜色。
settings-inspection-histogram-channels = 直方图通道
settings-inspection-histogram-channels-help = 直方图显示哪些曲线。
settings-inspection-histogram-channels-colour = 红、绿、蓝
settings-inspection-histogram-channels-luminance = 亮度
settings-inspection-histogram-channels-both = 颜色和亮度
settings-inspection-histogram-log = 对数直方图
settings-inspection-histogram-log-help = 以对数比例显示直方图，使较小的数量仍然可见。

## Channel isolation (machine-drafted, needs native review)
palette-channel-red = 显示红色通道
palette-channel-green = 显示绿色通道
palette-channel-blue = 显示蓝色通道
palette-channel-alpha = 显示 Alpha 通道（再按：棋盘格背景，然后纯色背景）
palette-channel-luma = 显示亮度
channel-normal = 所有通道
channel-red = 红色通道
channel-green = 绿色通道
channel-blue = 蓝色通道
channel-alpha = Alpha 通道
channel-alpha-checker = 棋盘格上的透明度
channel-alpha-matte = 纯色上的透明度
channel-luma = 亮度
disabled-no-alpha = 此图像没有透明度
settings-inspection-alpha-matte-color = Alpha 背景色
settings-inspection-alpha-matte-color-help = 在“纯色上的透明度”通道视图中，图像背后的纯色。

## Colour-vision-safe display: false colour, colour bars, channel colours, CVD simulation, HiLo
palette-false-colour = 显示或隐藏伪彩色
palette-next-colour-map = 下一个色图
palette-channel-colours = 下一组通道颜色（红–绿、品红–绿、品红–黄–青）
palette-hilo = 显示或隐藏 HiLo（位深最大值和最小值处的像素）
palette-colour-vision = 下一种色觉模拟
palette-colour-vision-off = 停止色觉模拟
palette-colour-vision-protanopia = 模拟红色盲（无红视锥）
palette-colour-vision-deuteranopia = 模拟绿色盲（无绿视锥）
palette-colour-vision-tritanopia = 模拟蓝色盲（无蓝视锥）
palette-colour-vision-achromatopsia = 模拟全色盲（无颜色）
# Colour map names. Proper names of published maps; usually left untranslated.
colour-map-viridis = Viridis
colour-map-cividis = Cividis
colour-map-batlow = Batlow
colour-map-roma = Roma
colour-map-vik = Vik
colour-map-oslo = Oslo
colour-map-lajolla = Lajolla
colour-map-thermal = Thermal
colour-map-haline = Haline
colour-map-solar = Solar
colour-map-ice = Ice
colour-map-deep = Deep
colour-map-dense = Dense
colour-map-algae = Algae
colour-map-rain = Rain
colour-map-speed = Speed
colour-map-balance = Balance
colour-map-curl = Curl
colour-map-phase = Phase
colour-map-jet = Jet（对色觉缺陷不友好）
# The colour bar's title. $map is a colour map name, $unit the value unit (HU, counts…).
colour-bar-title = { $map }
colour-bar-title-unit = { $map } · { $unit }
# Unit of raw sample values without a physical unit.
colour-bar-unit-raw = 原始值
# Screen reader summary of the colour bar.
colour-bar-a11y = 色条 { $map }：{ $low } 至 { $high } { $unit }
# Badges at the top of the window while a display aid is on (also announced).
badge-false-colour = 伪彩色：{ $map }
badge-hilo = HiLo：标记最大值和最小值处的像素
badge-colour-vision-protanopia = 正在模拟红色盲
badge-colour-vision-deuteranopia = 正在模拟绿色盲
badge-colour-vision-tritanopia = 正在模拟蓝色盲
badge-colour-vision-achromatopsia = 正在模拟全色盲
channel-colours-red-green = 通道：红、绿、蓝
channel-colours-magenta-green = 通道：品红、绿
channel-colours-magenta-yellow-cyan = 通道：品红、黄、青
settings-inspection-false-colour-map = 伪彩色色图
settings-inspection-false-colour-map-help = 用于伪彩色及其色条的色图。除 jet 和 phase 外，所有色图对任何色觉都清晰可读。
settings-inspection-false-colour-map-viridis = Viridis
settings-inspection-false-colour-map-cividis = Cividis
settings-inspection-false-colour-map-batlow = Batlow
settings-inspection-false-colour-map-roma = Roma（发散）
settings-inspection-false-colour-map-vik = Vik（发散）
settings-inspection-false-colour-map-oslo = Oslo
settings-inspection-false-colour-map-lajolla = Lajolla
settings-inspection-false-colour-map-thermal = Thermal
settings-inspection-false-colour-map-haline = Haline
settings-inspection-false-colour-map-solar = Solar
settings-inspection-false-colour-map-ice = Ice
settings-inspection-false-colour-map-deep = Deep
settings-inspection-false-colour-map-dense = Dense
settings-inspection-false-colour-map-algae = Algae
settings-inspection-false-colour-map-rain = Rain
settings-inspection-false-colour-map-speed = Speed
settings-inspection-false-colour-map-balance = Balance（发散）
settings-inspection-false-colour-map-curl = Curl（发散）
settings-inspection-false-colour-map-phase = Phase（循环）
settings-inspection-false-colour-map-jet = Jet（对色觉缺陷不友好）
settings-inspection-colour-bar = 色条
settings-inspection-colour-bar-help = 伪彩色开启时，显示以图像自身单位标注数值的色条。
settings-inspection-channel-colours = 通道颜色
settings-inspection-channel-colours-help = 颜色通道的显示方式。品红–绿和品红–黄–青在红绿色盲下仍可区分。
settings-inspection-channel-colours-red-green = 红、绿、蓝
settings-inspection-channel-colours-magenta-green = 品红、绿
settings-inspection-channel-colours-magenta-yellow-cyan = 品红、黄、青
settings-inspection-colour-vision = 色觉模拟
settings-inspection-colour-vision-help = 按色觉缺陷者所见显示整个窗口（Machado 等，2009）。导出永不受影响。
settings-inspection-colour-vision-off = 关闭
settings-inspection-colour-vision-protanopia = 红色盲
settings-inspection-colour-vision-deuteranopia = 绿色盲
settings-inspection-colour-vision-tritanopia = 蓝色盲
settings-inspection-colour-vision-achromatopsia = 全色盲
settings-inspection-auto-range = 高位深灰度自动范围
settings-inspection-auto-range-help = 将 16 位灰度图像（TIFF、PNG、FITS）从最暗值拉伸到最亮值。DICOM 图像保留自身的窗。

## Keyboard crosshair and the value under the cursor (machine-drafted, needs native review)

crosshair-on = 十字线已开启。方向键移动，Shift 加大步长，Enter 点击，Esc 结束。
crosshair-off = 十字线已关闭
announce-cursor-nothing = 光标下没有内容
