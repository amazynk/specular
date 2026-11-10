# Specular — Scientific images (FITS, XISF): stretch, limits, planes, channels, coordinates. 简体中文.

keys-mode-science = 科学图像
cheatsheet-science = 科学图像
cheatsheet-science-look = 拉伸与显示范围
cheatsheet-science-planes = 平面与通道
# machine-drafted, needs native review
cheatsheet-science-channels-1-9 = 显示或隐藏通道 1 至 9
cheatsheet-science-channels-10-16 = 显示或隐藏通道 10 至 16
palette-science-stretch = 下一种拉伸
palette-science-limits = 下一种显示范围
palette-science-next-plane = 下一平面
palette-science-prev-plane = 上一平面
palette-science-next-axis = 逐步浏览另一轴
palette-science-composite = 将通道合成为彩色
palette-science-channel-1 = 显示或隐藏通道 1
palette-science-channel-2 = 显示或隐藏通道 2
palette-science-channel-3 = 显示或隐藏通道 3
palette-science-channel-4 = 显示或隐藏通道 4
palette-science-channel-5 = 显示或隐藏通道 5
palette-science-channel-6 = 显示或隐藏通道 6
palette-science-channel-7 = 显示或隐藏通道 7
palette-science-channel-8 = 显示或隐藏通道 8
palette-science-channel-9 = 显示或隐藏通道 9
# machine-drafted, needs native review
palette-science-channel-10 = 显示或隐藏通道 10
palette-science-channel-11 = 显示或隐藏通道 11
palette-science-channel-12 = 显示或隐藏通道 12
palette-science-channel-13 = 显示或隐藏通道 13
palette-science-channel-14 = 显示或隐藏通道 14
palette-science-channel-15 = 显示或隐藏通道 15
palette-science-channel-16 = 显示或隐藏通道 16
palette-science-grid = 显示或隐藏坐标网格
palette-science-reset = 自动外观
palette-science-level = 调平高度图（减去平均平面）
palette-science-wiggle = 在颜色之上叠加地震波形道（波形加变面积）
# machine-drafted, needs native review
palette-science-wireframe = 在颜色之上叠加非结构网格的边（线框）
palette-science-import-lut = 从文件导入颜色表 (ImageJ .lut)…
palette-science-hillshade = 为高程模型添加地形阴影（山体阴影）
palette-science-fft = 平面的功率谱 (FFT)
palette-science-catalogue = 叠加源星表 (VOTable、FITS 表、CSV)…
palette-science-clear-catalogues = 移除叠加的星表
# A catalogue overlaid on a sky image: its name (the table's or the file's), how many of its
# sources fall on the image, and how many of those are drawn (at most a set number).
science-catalogue-toast = { $name }：{ $count ->
       *[other] 图像上有 { $count } 个源
    }
science-catalogue-capped = { $name }：显示图像上 { $count } 个源中的 { $shown } 个
science-catalogue-none = { $name }：图像上没有源
science-catalogue-failed = 不是含天球位置的星表：{ $name }
science-catalogues-cleared = 已移除星表
# The file dialog's filter name for catalogue files.
science-catalogue-filter = 源星表
# A GW sky map's credible region: the smallest area holding this share of the probability,
# drawn as a contour labelled e.g. "90 %".
science-credible = { PERCENT($percent) }
palette-science-ion-image = 指定 m/z 的离子图像 (质谱)…
disabled-not-science = 仅适用于科学图像（天文、电子显微镜和扫描探针显微镜）
disabled-one-plane = 图像只有一个平面
disabled-one-axis = 图像没有可逐步浏览的其他轴
disabled-no-channels = 图像没有可合成的此通道
disabled-no-coordinates = 图像没有天球坐标
disabled-not-height-map = 仅适用于高度图（Gwyddion、Igor）
disabled-not-elevation = 仅适用于单波段的地理参考高程模型
disabled-not-seismic = 仅适用于地震数据（SEG-Y、Seismic Unix）
disabled-not-mass-spec = 仅适用于质谱图像（imzML）
# machine-drafted, needs native review
disabled-not-ugrid = 仅适用于非结构网格（UGRID、ICON 和 MPAS NetCDF）
disabled-not-bayer = 仅适用于彩色相机原始帧（带拜耳阵列的 FITS、SER 视频）
palette-debayer = 以去马赛克或原始方式显示彩色相机帧
palette-bayer-pattern = 此文件的下一个拜耳阵列
science-debayer-on = 已去马赛克（{ $pattern }）
science-debayer-off = 原始彩色滤镜阵列
science-bayer-pattern = 拜耳阵列 { $pattern }
# machine-drafted, needs native review
palette-annotations = 显示或隐藏标注
disabled-no-annotations = 图像旁没有标注文件（NDPA、Aperio 或 ASAP XML）
science-annotations-on = 标注已显示
science-annotations-off = 标注已隐藏
science-stretch-auto = 自动（每种颜色）
# Stretches: how values between the limits map to grey levels.
science-stretch-linear = 线性
science-stretch-sqrt = 平方根
science-stretch-log = 对数
science-stretch-asinh = Asinh
science-stretch-histeq = 直方图均衡化
# Limits: which values show black and white. $percent is e.g. 99.5.
science-limits-zscale = Zscale
science-limits-minmax = 最小/最大
science-limits-percentile = { PERCENT($percent) }
# Seismic gain: limits symmetric about zero at the $percent percentile of the amplitudes.
science-limits-symmetric = ±{ PERCENT($percent) }
# The display range the file stores (ImageJ's min and max), used until other limits are chosen.
science-limits-file = 文件中的范围
# Toasts when the look changes.
science-stretch-toast = 拉伸：{ $stretch }
science-limits-toast = 显示范围：{ $limits }
science-reset-toast = 自动外观
# An imported colour table: $name is its file name.
science-lut-toast = 颜色表 { $name }
science-lut-failed = 不是颜色表：{ $name }
# The file dialog's filter name for .lut files.
science-lut-filter = ImageJ 颜色表
science-composite-on = 通道已合成为彩色
science-composite-off = 单通道灰度
science-channel-on = 已显示 { $channel }
science-channel-off = 已隐藏 { $channel }
# machine-drafted, needs native review
# A channel without a name in the file, in the shown/hidden toasts.
science-channel-unnamed = 通道 { $number }
science-grid-on = 坐标网格已显示
science-grid-off = 坐标网格已隐藏
# X-ray diffraction frames: the same action shows circles of equal resolution (d-spacing).
science-rings-on = 分辨率环已显示
science-rings-off = 分辨率环已隐藏
# A resolution ring's label: its d-spacing in ångström, e.g. "2.5 Å".
science-ring = { $d } Å
# Seismic wiggle traces: drawn once the traces are at least 4 screen pixels apart.
science-wiggle-on = 波形道已开启（放大即可看到）
science-wiggle-off = 波形道已关闭
# machine-drafted, needs native review
# Unstructured meshes: the edges drawn over the faces; too many to draw in view until the
# user zooms in.
science-wireframe-on = 网格边已开启
science-wireframe-off = 网格边已关闭
science-wireframe-dense = 网格边已开启（放大后可见）
# Plane levelling (scanning probe height maps): the best-fit plane is subtracted for display.
science-level-on = 平面调平已开启
science-level-off = 平面调平已关闭
# Relief shading of an elevation model. $azimuth is the sun's direction in degrees clockwise
# from north, $altitude its height above the horizon in degrees.
science-hillshade-on = 山体阴影已开启：太阳方位 { $azimuth }°，高度 { $altitude }°
science-hillshade-off = 山体阴影已关闭
# The power spectrum view: the plane's centred 2-D Fourier power, log scaled.
science-fft-on = 功率谱已显示
science-fft-off = 功率谱已隐藏
science-axis-toast = 逐步浏览：{ $axis }
# Readout lines (bottom corner).
# The value under the pointer, with the values' unit when the file states one.
science-value = 值 { $value }
science-value-unit = 值 { $value } { $unit }
# machine-drafted, needs native review
science-value-flags = 值 { $value }（位 { $bits }）
# machine-drafted, needs native review
# A class value under the pointer and its name, e.g. "Value 4 · Vegetation".
science-value-class = 值 { $value } · { $class }
# machine-drafted, needs native review
# Sentinel-2 scene classes (L2A SCL, ESA's names) and L1C cloud masks.
s2-class-no-data = 无数据
s2-class-defective = 饱和或缺陷
s2-class-dark = 暗区像素
s2-class-cloud-shadow = 云阴影
s2-class-vegetation = 植被
s2-class-not-vegetated = 非植被
s2-class-water = 水体
s2-class-unclassified = 未分类
s2-class-cloud-medium = 中概率云
s2-class-cloud-high = 高概率云
s2-class-cirrus = 薄卷云
s2-class-snow = 雪或冰
s2-class-clear = 晴空
s2-class-opaque-cloud = 不透明云
# Several channels under the pointer: "R 12.5  G 3.1  B 0.4".
science-values = { $values }
# Sky position under the pointer, sexagesimal then degrees.
science-radec = 赤经 { $ra }  赤纬 { $dec }
science-radec-deg = { $ra }°  { $dec }°
# Galactic or ecliptic longitude and latitude.
science-lonlat = l { $lon }°  b { $lat }°
science-wcs-unsupported = 不支持的 WCS：{ $reason }
# machine-drafted, needs native review
# A B1950 (FK4) image: what is converted between its frame and another. Its own position
# stays B1950 in the readout. ICRS, FK4 and SAMP are names and stay as they are.
science-fk4-markers = 目录标记：已从 ICRS → FK4 转换
science-fk4-markers-galactic = 目录标记：已从银道坐标 → FK4 转换
science-fk4-samp = SAMP：已从 FK4 → ICRS 转换
# The look: stretch and limits.
science-look = { $stretch } · { $limits }
# Shown under the look while plane levelling is on.
science-levelled = 已平面调平
# Shown under the look while the hillshade is on.
science-hillshaded = 山体阴影：太阳方位 { $azimuth }°，高度 { $altitude }°
# Shown in the readout while the power spectrum is shown.
science-fft = 功率谱 (log₁₀)
# The spectrum's value under the pointer: log₁₀ of the power.
science-fft-power = log₁₀ 功率 { $value }
# Spatial frequency under the pointer along x and y, and its unit ("1/µm", "1/px").
science-fft-frequency = 频率 u { $u }  v { $v } { $unit }
# The period of that frequency: "Period 2.5 nm".
science-fft-period = 周期 { $period }
# Crystal orientation under the pointer (EBSD): Bunge Euler angles in degrees.
science-euler = φ₁ { $phi1 }  Φ { $phi }  φ₂ { $phi2 }
# The crystal phase under the pointer and its symmetry ("Iron fcc (m-3m)").
science-phase = 物相 { $phase } ({ $symmetry })
# Title of the spectrum plot of the pixel under the pointer (an EDS map).
science-spectrum-title = { $x }, { $y } 处的谱图
# Physical position under the pointer (microscopy): "x 1.25 µm  y 3.5 µm".
science-position = x { $x }  y { $y }
# machine-drafted, needs native review
# CERN ROOT histograms: the bin under the pointer on one axis. $axis is the axis title
# ("p_{T}", or "x"), $range the bin's label or its edges ("20 GeV – 25 GeV"), $bin its
# number counted from 1.
science-root-bin = { $axis } { $range }（第 { $bin } 个 bin）
# A ROOT graph's point under the pointer.
science-root-point = x { $x }  y { $y }
# Under- and overflow bins (fills outside the axis ranges) hold something but are not shown.
science-root-flow = 已隐藏 { $count } 个下溢/上溢 bin（总和 { $sum }）
# Bins of different widths are drawn on an even grid of pixels.
science-root-resampled = 可变宽度 bin 已重采样为均匀网格
# machine-drafted, needs native review
# A flow cytometry plot (FCS) under the pointer: each axis's parameter and its value.
science-cytometry-at = { $x } { $xv }  { $y } { $yv }
# A cytometry histogram under the pointer: its parameter and the value.
science-cytometry-at-x = { $x } { $xv }
# Events in the plot's bin (or the histogram's bar) under the pointer.
science-cytometry-events = { $count ->
   *[other] { $count } 个事件
}
# The plot's axis scales ($x, $y: Linear, Log, Logicle or Arcsinh).
science-cytometry-scales = x { $x }, y { $y }
science-cytometry-scale-x = x { $x }
# Spillover compensation is applied, from the file's keyword ($keyword, e.g. $SPILLOVER).
science-cytometry-compensated = 已补偿 ({ $keyword })
science-cytometry-uncompensated = 未补偿
# X-ray diffraction frames: what lies under the pointer. q is the momentum transfer
# (per ångström), d the lattice spacing (ångström), 2θ the scattering angle. The symbols
# stay as they are: "q 0.512 Å⁻¹  d 12.27 Å  2θ 4.56°".
science-scatter = q { $q } Å⁻¹  d { $d } Å  2θ { $tth }°
# Seismic readout: the trace under the pointer in a 3-D volume ($inline, $crossline the
# header numbers) or a 2-D line; $trace counts from 1 in file order, $cdp is the CDP number.
science-seismic-cell = 主测线 { $inline }  联络测线 { $crossline }
science-seismic-trace = 道 { $trace }  CDP { $cdp }
# A cell of a 3-D volume with no trace in the file.
science-seismic-gap = 无道
# Two-way time (ms) or depth of the row under the pointer.
science-seismic-time = 时间 { $time } ms
science-seismic-depth = 深度 { $depth } { $unit }
# The trace's CDP (or source) X and Y with the coordinate scalar applied.
science-seismic-xy = X { $x }  Y { $y }
# machine-drafted, needs native review
# Unstructured mesh readout: the face (cell) under the pointer, counting from 0 as in the
# file's connectivity, and the node nearest to the pointer when the values sit on nodes.
science-mesh-face = 面 { $face }
science-mesh-face-node = 面 { $face }  节点 { $node }
# The pointer lies between the mesh's faces (land in an ocean mesh, outside its boundary).
science-mesh-outside = 网格之外
# Position on an axis past the image: "Plane 3 of 40", or with its world value.
science-plane = { $axis } { $index }/{ $count }
science-plane-world = { $axis } { $index }/{ $count }（{ $world }）
science-axis-unnamed = 平面
science-axis-extension = 图像
science-axis-channel = 通道
# A column of a HEALPix sky map (probability, distance…) shown as the image.
science-axis-field = 字段
# X-ray event lists binned into an image: the bin size, in the event list's sky pixels
# ("Bin 2 of 4 (8 px)"), and the energy band of the events counted.
science-axis-bin = 分箱
science-bin-size = { $size } px
science-axis-band = 能量
science-band-all = 全部能量
# machine-drafted, needs native review
# Complex-valued arrays (NumPy, MATLAB, Zarr, TDMS): which part of each value is shown.
science-axis-part = 分量
science-part-magnitude = 模
science-part-real = 实部
science-part-imaginary = 虚部
science-part-phase = 相位（弧度）
# machine-drafted, needs native review
# Complex radar (SAR) images: intensity in decibels.
science-part-db = 强度（dB）
# machine-drafted, needs native review
# The frames of a thermal sequence (FLIR SEQ/CSQ), in time.
science-axis-frame = 帧
science-readout = 值与坐标
# Accessible names of the plane sliders.
science-sliders = 平面
# The scale bar's length.
science-scale-arcsec = { $value }″
science-scale-arcmin = { $value }′
science-scale-deg = { $value }°
# A physical length, e.g. "20 nm" (electron microscopy images).
science-scale-length = { $value } { $unit }
# machine-drafted, needs native review
# A length along a named axis of a plot, e.g. "20 GeV (p_{T})".
science-scale-axis = { $length } ({ $axis })
science-scale-a11y = 比例尺：{ $length }
# Mass spectrometry images (imzML). The image is the total ion current (each spot's summed
# intensities) until an m/z is typed; then that ion's image, its intensities summed over
# m/z ± the tolerance. $window is e.g. "760.5851 ± 5 ppm".
science-ion-tic = 总离子流
science-ion-window = 离子图像 m/z { $window }
# The m/z prompt: $text is what is typed so far (opens on the base peak under the pointer).
science-ion-prompt = 离子图像 m/z：{ $text }▏
# $tolerance is e.g. "10 ppm" or "0.1 Da"; a tolerance can also be typed after the m/z
# ("760.585 5 ppm"). Tab steps the tolerance; Enter on an empty prompt shows the total ion
# current again.
science-ion-prompt-hint = ± { $tolerance } · Tab 容差 · Enter 显示 · Esc 取消
science-ion-bad = 不是有效的 m/z：{ $text }
# The spot under the pointer as the file numbers it (from 1).
science-ms-spot = 采样点 x { $x }  y { $y }
science-ms-spot-z = 采样点 x { $x }  y { $y }  z { $z }
science-ms-no-spectrum = 该采样点没有谱图
# machine-drafted, needs native review
palette-science-temperature-unit = 温度单位 (°C、°F、K)
palette-science-isotherm = 高于或低于指针处温度的等温线
palette-science-thermal-parameters = 发射率和反射温度…
palette-science-export-temperatures = 导出温度 (浮点 TIFF 或 CSV)…
palette-thermal-visible = 热图像的可见光照片
disabled-not-thermal = 仅适用于热图像
disabled-not-radiometric = 该文件存储的是温度：其参数无法更改
disabled-no-visible-photo = 该热图像没有可见光照片
science-temperature-unit-toast = 温度单位：{ $unit }
science-isotherm-above = 高于 { $level } 的等温线
science-isotherm-below = 低于 { $level } 的等温线
science-isotherm-off = 等温线已关闭
science-thermal-emissivity-prompt = 发射率：{ $text }
science-thermal-reflected-prompt = 反射温度 ({ $unit })：{ $text }
science-thermal-prompt-hint = Tab 下一字段 · Enter 重新计算 · Esc 取消
science-thermal-bad = 不是有效的值：{ $text }
science-thermal-applied = 已按发射率 { $emissivity }、反射温度 { $reflected } 重新计算
science-thermal-conditions = 发射率 { $emissivity } · 反射温度 { $reflected }
science-thermal-export-tiff = 32 位浮点 TIFF
science-thermal-export-csv = CSV
science-thermal-export-done = 温度已保存到 { $name }
science-thermal-export-failed = 无法保存温度：{ $reason }
# machine-drafted, needs native review
science-values-failed = 无法显示数值：{ $reason }
# machine-drafted, needs native review
# Thermal sequences (FLIR SEQ/CSQ): playback and each measurement over time.
palette-science-thermal-play = 播放或暂停热图像序列
palette-science-thermal-series-csv = 保存随时间变化的温度 (CSV)…
disabled-not-thermal-sequence = 仅适用于热图像序列
thermal-seq-title = 随时间变化的温度
# $time is seconds from the first frame, e.g. "0.12 s".
thermal-seq-frame = 第 { $frame }/{ $frames } 帧 · { $time }
thermal-seq-play = 播放
thermal-seq-pause = 暂停
thermal-seq-playing = 正在播放
thermal-seq-paused = 已暂停
thermal-seq-save-csv = 保存 CSV…
thermal-seq-hint = 添加测温点、线或区域 (M)，即可绘制其随时间的变化。
thermal-seq-reading = 正在读取第 { $done }/{ $total } 帧…
thermal-seq-failed = 无法读取帧：{ $reason }
thermal-seq-no-series = 暂无可保存的内容：请添加测温点、线或区域。
# For screen readers: each measurement's value at the frame shown, e.g.
# "1 Spot: 36.4 °C, 2 Rectangle: 30.1 °C".
thermal-seq-chart = 图表：{ $values }
# machine-drafted, needs native review
# The report (PDF) of the image shown: a thermal report for a thermal image, an inspection report for any other.
palette-thermal-report = 报告 (PDF)…
thermal-report-heading = 热成像报告
thermal-report-default-title = 红外热成像检测报告
thermal-report-title-label = 标题
thermal-report-notes-label = 备注
thermal-report-photo = 包含可见光照片
thermal-report-folder = 添加此文件夹中的其他热图像
thermal-report-save = 保存 PDF…
thermal-report-busy = 正在生成报告…
thermal-report-nothing = 尚未显示温度。
thermal-report-saved = 报告已保存到 { $name }（{ $count ->
   *[other] { $count } 张图像
}）
thermal-report-failed = 无法保存报告：{ $reason }
# In the PDF.
thermal-report-notes = 备注
thermal-report-measurements = 测量
thermal-report-no-measurements = 无测量。
thermal-report-thermal-caption = 热图像
thermal-report-visible-caption = 可见光照片
thermal-report-col-measurement = 测量
thermal-report-col-min = 最小值
thermal-report-col-max = 最大值
thermal-report-col-mean = 平均值
thermal-report-col-emissivity = 发射率
thermal-report-whole-image = 整幅图像
thermal-report-camera = 相机
thermal-report-lens = 镜头
thermal-report-date = 拍摄时间
thermal-report-place = 地点
thermal-report-frame = 帧
thermal-report-emissivity = 发射率
thermal-report-reflected = 反射温度
thermal-report-distance = 距离
thermal-report-humidity = 湿度
thermal-report-range = 范围
thermal-report-format = 格式
thermal-report-page = 第 { $page }/{ $pages } 页
