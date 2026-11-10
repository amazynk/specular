# Specular — Medical images (DICOM). machine-drafted, needs native review

cheatsheet-medical = 医学影像
palette-window-level = 窗宽/窗位工具（左键拖动）
palette-window-default = 窗: 文件默认值
palette-window-lung = 窗: 肺 (W 1500, L −600)
palette-window-mediastinum = 窗: 纵隔 (W 350, L 50)
palette-window-bone = 窗: 骨 (W 1800, L 400)
palette-window-brain = 窗: 脑 (W 80, L 40)
palette-window-stroke = 窗: 卒中 (W 40, L 40)
palette-window-soft-tissue = 窗: 软组织 (W 400, L 50)
palette-window-liver = 窗: 肝 (W 150, L 30)
palette-window-subdural = 窗: 硬膜下 (W 200, L 75)
palette-window-full-range = 窗: 全范围
palette-invert-grey = 反转灰度
palette-next-slice = 下一帧
palette-prev-slice = 上一帧
disabled-not-medical = 仅适用于灰度医学影像
disabled-one-frame = 图像只有一帧
settings-section-medical = 医学影像
settings-medical-keep-window = 下一个序列保留窗宽/窗位
settings-medical-keep-window-help = 切换到其他序列时保留你设置的窗，而不是从其默认值开始。同一文件的各帧和同一序列的各层始终共用。
medical-window = C { $center }  W { $width }
medical-inverted = 已反转
medical-frame = 第 { $frame } 帧，共 { $count } 帧
medical-tool = 窗宽/窗位工具：拖动
medical-tool-on = 窗宽/窗位工具已开启：拖动以调整
medical-tool-off = 窗宽/窗位工具已关闭
medical-preset-not-ct = CT 预设需要以亨氏单位表示的值
medical-file-window-none = 此文件中没有窗口 { $index }
medical-file-window-unnamed = 文件窗口 { $index }/{ $count }
medical-file-window = 文件窗口 { $index }/{ $count }：{ $name }
measure-templates-need-medical = 几何模板随医学配置文件提供
medical-off = 此图像无法使用医学工具：{ $reason }
medical-readout = 值和窗

## Measurements, cine and overlays (machine-drafted, needs native review)

palette-cine = 播放帧 (电影)
palette-measure-length = 测量长度
palette-measure-angle = 测量角度
palette-measure-ellipse = 测量椭圆区域
palette-measure-rectangle = 测量矩形区域
palette-clear-measurements = 清除测量
palette-measure-templates = 几何模板…
palette-overlays = 显示或隐藏叠加层
medical-cine = 正在以 { $rate } 帧/秒播放
medical-overlays-shown = 叠加层已显示
medical-overlays-hidden = 叠加层已隐藏
measure-hint-length = 长度:拖动或单击两端
measure-hint-angle = 角度:单击三个点,第二个为顶点
measure-hint-ellipse = 椭圆:在区域上拖动
measure-hint-rectangle = 矩形:在区域上拖动
measure-off = 测量工具已关闭
measure-cleared = 测量已清除
measure-length-mm = { $value } mm
# machine-drafted, needs native review
measure-length-angstrom = { $value } Å
measure-length-px = { $value } px
measure-area-mm = 面积 { $value } mm²
measure-area-px = 面积 { $value } px²
measure-angle = { $value }°
measure-mean = 平均值 { $mean } ± { $sd } { $unit }
measure-range = 最小 { $min }  最大 { $max }  ({ $count } px)
measure-caveat-detector = 位于探测器处:未校正放大率
measure-caveat-magnification = 估计的放大率校正
measure-caveat-uncalibrated = 无像素间距:以像素为单位
measure-caveat-outside-region = 在已校准的超声区域之外:以像素为单位

## DICOM orientation labels, presentation states and wheel stepping (machine-drafted, needs native review)
settings-medical-orientation-labels = 显示方位标记
settings-medical-orientation-labels-help = 图像边缘表示患者方位的字母（R/L 右/左，A/P 前/后，S/I 头/足），取自文件，并随旋转和翻转而变化。按 L 可在本次会话中切换显示。
settings-medical-wheel-steps-frames = 鼠标滚轮逐帧切换
settings-medical-wheel-steps-frames-help = 在含多帧的文件或序列中，滚轮在帧之间切换，而不是缩放或切换图像。Ctrl+滚轮仍可缩放。
# machine-drafted, needs native review
settings-medical-display-function = 灰阶显示
settings-medical-display-function-help = 医学图像灰阶的显示方式。DICOM GSDF（灰阶标准显示函数）在显示器的黑色与白色亮度之间按等可察觉的步长分配灰阶（DICOM PS3.14），与经过校准的诊断显示器一致。显示器的颜色描述文件仍会应用。
settings-medical-display-function-srgb = 标准（sRGB）
settings-medical-display-function-gsdf = DICOM GSDF
settings-medical-display-black = 显示器黑色亮度
settings-medical-display-black-help = 显示器黑色的亮度（包括环境光），来自测量或显示器的规格书。供 DICOM GSDF 使用。
settings-medical-display-white = 显示器白色亮度
settings-medical-display-white-help = 显示器白色的亮度，来自测量或显示器的规格书。供 DICOM GSDF 使用。
palette-orientation-labels = 显示或隐藏方位标记
palette-presentation-state = 下一个显示状态
disabled-no-presentation-state = 没有引用此图像的显示状态
medical-labels-shown = 方位标记已显示
medical-labels-hidden = 方位标记已隐藏
medical-presentation = 显示状态：{ $label }（{ $index } / { $count }）
medical-presentation-none = 显示状态已关闭
medical-presentation-readout = 显示状态：{ $label }
medical-preset-presentation = 由显示状态的查找表设置灰阶

## Orientation labels for screen readers (machine-drafted, needs native review)
medical-orientation-a11y = 方位：左 { $left }，右 { $right }，上 { $top }，下 { $bottom }
