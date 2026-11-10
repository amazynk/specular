# machine-drafted, needs native review
# Specular 元数据面板——简体中文。约定见 i18n/en/metadata.ftl；数字与日期已由 ICU4X 按区域设置格式化。

## 摘要分组标题

group-camera = 相机
group-exposure = 曝光
group-lens = 镜头
group-date = 日期和时间
group-location = 位置
# machine-drafted, needs native review
# A DJI drone photo: the gimbal's and the aircraft's attitude, the height above take-off.
group-drone = 无人机
label-gimbal = 云台
label-flight = 飞行器
label-relative-altitude = 相对起飞点高度
drone-attitude = 偏航 { $yaw }，俯仰 { $pitch }，横滚 { $roll }
# A DJI thermal camera's radiometric JPEG: its photo opens, its temperatures can't.
label-thermal-data = 温度
drone-thermal-refused = 不显示：DJI 的 SDK 许可不允许
drone-thermal-refused-detail = 读取 DJI 温度需要 DJI Thermal SDK，而其许可只允许随附该 SDK 的程序使用它。已显示图像及其元数据。
group-image = 图像
group-file = 文件
group-rights = 版权与描述
group-credentials = 内容凭证
# machine-drafted, needs native review
group-medical = 医学图像
# machine-drafted, needs native review
group-inspection = 部件与检测
group-nde-image = 检测图像（DICONDE）
# machine-drafted, needs native review
# Industrial CT volumes (Zeiss Xradia, Amira, Bruker SkyScan, Nikon X-Tek, VGI, raw volumes).
group-industrial-image = 检测图像
group-science = 科学图像
group-em = 电子显微镜
group-spm = 扫描探针显微镜
group-slide = 玻片
group-microscopy = 显微镜
# X-ray diffraction detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS).
group-diffraction = 衍射
group-surface = 表面形貌
group-ebsd = 电子背散射衍射
group-seismic = 地震数据
# machine-drafted, needs native review
group-asdf = ASDF
# machine-drafted, needs native review
# CERN ROOT files (particle physics histograms and graphs).
group-root = ROOT
# machine-drafted, needs native review
# CASA images (radio interferometry: the .image, .residual, .model directories tclean writes).
group-casa = CASA 图像
# machine-drafted, needs native review
# GIFTI files: brain surfaces and the values on their vertices (Human Connectome Project
# and FreeSurfer exports).
group-gifti = GIFTI
# CIFTI-2 files: maps over cortical surfaces and subcortical voxels ("grayordinates").
group-cifti = CIFTI
# White-matter fibre tracts (TrackVis, MRtrix).
group-tractography = 纤维束成像
# machine-drafted, needs native review
group-annotations = 标注
# Imaging mass spectrometry (imzML): a mass spectrum per pixel.
group-mass-spec = 质谱成像
# machine-drafted, needs native review
group-thermal = 热图像
# machine-drafted, needs native review
# Flow cytometry list-mode data (FCS): events of a cell sorter or analyser.
group-cytometry = 流式细胞术
# machine-drafted, needs native review
group-spectroscopy = 光谱学
# machine-drafted, needs native review
# Unstructured meshes of ocean, coastal and climate models (UGRID, ICON, MPAS in NetCDF).
group-ugrid = 非结构网格
group-drawing = 图纸
# machine-drafted, needs native review
group-volume = 容积

## 摘要项目标签

label-camera = 相机
label-software = 软件
label-application = 应用程序
label-shutter = 快门速度
label-aperture = 光圈
label-iso = ISO
label-focal-length = 焦距
label-exposure-bias = 曝光补偿
label-program = 模式
label-metering = 测光模式
label-white-balance = 白平衡
label-flash = 闪光灯
label-lens = 镜头
label-lens-make = 镜头制造商
label-lens-range = 镜头规格
# machine-drafted, needs native review
label-focus-mode = 对焦模式
label-stabilisation = 图像稳定
label-taken = 拍摄时间
label-digitised = 数字化时间
label-edited = 最后编辑
label-place = 地点
label-coordinates = 坐标
label-altitude = 海拔
label-direction = 方向
label-accuracy = 精度
label-gps-time = GPS 时间
label-dimensions = 尺寸
label-encoding = 编码
# machine-drafted, needs native review
label-motion = 动态
motion-kind-motion-photo = 动态照片
motion-kind-micro-video = 动态照片 (MicroVideo)
motion-kind-samsung = 三星动态照片
motion-kind-live-photo = 实况照片
motion-duration = { $seconds } 秒
label-colour-profile = 颜色配置文件
label-orientation = 方向
label-file-name = 名称
label-folder = 文件夹
label-file-size = 大小
label-format = 格式
label-modified = 修改时间
label-title = 标题
label-description = 描述
label-creator = 创作者
label-copyright = 版权
label-keywords = 关键字
label-rating = 评级
label-colour-label = 颜色标签
label-credentials = 内容凭证

## “所有字段”分组标题（标签名称保持标准原文）

fields-ifd0 = IFD0（主图像）
fields-exif = Exif
fields-gps = GPS
fields-interop = 互操作性
fields-ifd1 = IFD1（缩略图）
fields-icc = ICC 配置文件头
fields-xmp = XMP：{ $namespace }（{ $prefix }）
xmp-namespace = { $prefix ->
    [dc] Dublin Core
    [xmp] 基本
    [xmpRights] 版权
    [xmpMM] 媒体管理
    [xmpDM] 动态媒体
    [tiff] TIFF
    [exif] Exif
    [exifEX] Exif 2.3+
    [aux] 相机附件
    [photoshop] Photoshop
    [crs] Camera Raw 设置
    [lr] Lightroom
    [Iptc4xmpCore] IPTC Core
    [Iptc4xmpExt] IPTC Extension
    [plus] PLUS 授权
    [GPano] 全景照片
    [GCamera] Google 相机
    [hdrgm] HDR 增益图
    [mwg-rs] 区域
    [pdf] PDF
    [stEvt] 资源事件
    [stRef] 资源引用
   *[other] 其他
}
unknown-tag = 未知标签 { $number }（{ $context }）
blob-bytes = { $num ->
   *[other] （{ $num } 字节）
}
maker-note-bytes = （{ $num } 字节，厂商注释，未解码）
more-values = { $shown }，…（共 { $total } 个值）

## 通用连接符

list-separator = {"，"}
one-line-separator = {" · "}

## 曝光

exposure-time-fraction = 1/{ $denominator } 秒
exposure-time-seconds = { $seconds } 秒
aperture-value = f/{ $f }
iso-value = ISO { $iso }
focal-length-value = { $mm } mm
focal-length-with-equiv = { $mm } mm（等效 { $equiv } mm）
focal-length-equiv-only = 等效 { $equiv } mm
exposure-bias-value = { $sign ->
    [plus] +{ $ev } EV
    [minus] −{ $ev } EV
   *[zero] 0 EV
}
exposure-program = { $value ->
    [1] 手动
    [2] 程序自动
    [3] 光圈优先
    [4] 快门优先
    [5] 创意（景深优先）
    [6] 运动（高速快门）
    [7] 人像
    [8] 风景
   *[other] 未知
}
metering-mode = { $value ->
    [1] 平均测光
    [2] 中央重点平均测光
    [3] 点测光
    [4] 多点测光
    [5] 矩阵（评价）测光
    [6] 局部测光
   *[other] 其他
}
# machine-drafted, needs native review
# Canon maker note CameraSettings FocusMode; 256–258 are the live-view variants of 0–2.
canon-focus-mode = { $value ->
    [0] 单次自动对焦
    [1] 人工智能伺服自动对焦
    [2] 人工智能自动对焦
    [3] 手动对焦
    [4] 单次
    [5] 连续
    [6] 手动对焦
    [16] 泛焦
    [256] 单次自动对焦（实时显示）
    [257] 人工智能伺服自动对焦（实时显示）
    [258] 人工智能自动对焦（实时显示）
    [512] 短片快照对焦
    [519] 短片伺服自动对焦
   *[other] 其他
}
# Canon maker note CameraSettings ImageStabilization; 256–260 repeat 0–4.
canon-stabilisation = { $value ->
    [0] 关闭
    [1] 开启
    [2] 仅拍摄时
    [3] 追随拍摄
    [4] 动态
    [256] 关闭
    [257] 开启
    [258] 仅拍摄时
    [259] 追随拍摄
    [260] 动态
   *[other] 其他
}
# machine-drafted, needs native review
panasonic-focus-mode = { $value ->
    [1] 自动
    [2] 手动对焦
    [4] 自动（对焦按钮）
    [5] 自动（连续）
    [6] AF-S
    [7] AF-C
    [8] AF-F
   *[other] 其他
}
# machine-drafted, needs native review
panasonic-stabilisation = { $value ->
    [2] 开（光学）
    [3] 关
    [4] 开（模式 2）
    [5] 开（光学、追随拍摄）
    [6] 开（仅机身）
    [7] 开（仅机身、追随拍摄）
    [9] Dual IS
    [10] Dual IS（追随拍摄）
    [11] Dual IS 2
    [12] Dual IS 2（追随拍摄）
   *[other] 其他
}
label-comment = 注释
fields-tags = { $kind } 标签
group-picture = 图像设置
fields-fujifilm = 富士制造商注释
label-film-simulation = 胶片模拟
label-dynamic-range = 动态范围
label-highlight-tone = 高光
label-shadow-tone = 阴影
label-colour-setting = 色彩
label-sharpness = 锐度
label-noise-reduction = 降噪
label-clarity = 清晰度
label-grain-effect = 颗粒效果
label-colour-chrome = 色彩效果
label-colour-chrome-blue = 彩色 FX 蓝色
label-shutter-type = 快门类型
label-drive-mode = 驱动模式
fuji-step = { $sign ->
    [plus] +{ $n }
    [minus] −{ $n }
   *[zero] 0
}
fuji-film-mode = { $value ->
    [0] Provia（标准）
    [256] 影楼人像
    [272] 影楼人像（增强饱和度）
    [288] Astia（柔和）
    [304] 影楼人像（增强锐度）
    [512] Velvia（鲜艳）
    [768] 影楼人像 Ex
    [1024] Velvia
    [1280] Pro Neg. Std
    [1281] Pro Neg. Hi
    [1536] Classic Chrome
    [1792] Eterna
    [2048] Classic Neg.
    [2304] Eterna Bleach Bypass
    [2560] Nostalgic Neg.
    [2816] Reala Ace
   *[other] 其他
}
fuji-monochrome = { $value ->
    [768] 黑白
    [769] 黑白，红色滤镜
    [770] 黑白，黄色滤镜
    [771] 黑白，绿色滤镜
    [784] 棕褐色
    [1280] Acros
    [1281] Acros，红色滤镜
    [1282] Acros，黄色滤镜
    [1283] Acros，绿色滤镜
   *[other] 黑白
}
fuji-white-balance = { $value ->
    [0] 自动
    [1] 自动（白色优先）
    [2] 自动（氛围优先）
    [256] 日光
    [512] 阴天
    [768] 日光荧光灯
    [769] 日光白荧光灯
    [770] 白色荧光灯
    [771] 暖白荧光灯
    [772] 居室暖白荧光灯
    [1024] 白炽灯
    [1280] 闪光灯
    [1536] 水下
    [3840] 自定义 1
    [3841] 自定义 2
    [3842] 自定义 3
    [3843] 自定义 4
    [3844] 自定义 5
    [4080] 色温
   *[other] 其他
}
fuji-white-balance-kelvin = { $kelvin } K
fuji-dynamic-range = DR{ $percent }
fuji-dynamic-range-auto = 自动（DR{ $percent }）
fuji-dynamic-range-setting = { $value ->
    [0] 自动
    [256] 标准（DR100）
    [512] 宽 1（DR230）
    [513] 宽 2（DR400）
   *[other] 其他
}
fuji-effect = { $value ->
    [0] 关
    [32] 弱
    [64] 强
   *[other] 其他
}
fuji-grain-size = { $value ->
    [16] 小
    [32] 大
   *[other] 其他
}
fuji-grain = { $strength }，{ $size }
fuji-shutter-type = { $value ->
    [0] 机械
    [1] 电子
    [2] 电子（长时间曝光）
    [3] 电子前帘
   *[other] 其他
}
fuji-drive-mode = { $value ->
    [0] 单张
    [1] 连拍（低速）
    [2] 连拍（高速）
   *[other] 其他
}
fuji-focus-mode = { $value ->
    [0] 手动
    [1] AF-S
    [2] AF-C
    [100] 自动
    [101] 手动
   *[other] 其他
}
fuji-stabilisation = { $kind }（{ $mode }）
fuji-stabilisation-kind = { $value ->
    [0] 无
    [1] 光学
    [2] 传感器位移
    [3] 镜头
    [258] 传感器位移和数码
    [512] 数码
   *[other] 其他
}
fuji-stabilisation-mode = { $value ->
    [0] 关
    [1] 开，连续
    [2] 开，仅拍摄时
   *[other] 其他
}
light-source = { $value ->
    [1] 日光
    [2] 荧光灯
    [3] 钨丝灯
    [4] 闪光灯
    [9] 晴天
    [10] 阴天
    [11] 阴影
    [12] 日光色荧光灯
    [13] 日白色荧光灯
    [14] 冷白色荧光灯
    [15] 白色荧光灯
    [16] 暖白色荧光灯
    [17] 标准光源 A
    [18] 标准光源 B
    [19] 标准光源 C
    [20] D55
    [21] D65
    [22] D75
    [23] D50
    [24] 影棚钨丝灯
   *[other] 其他
}
white-balance-auto = 自动
white-balance-manual = 手动
white-balance-manual-with-light = 手动（{ $light }）
flash-none = 无闪光灯功能
flash-fired = 闪光灯已闪光
flash-not-fired = 闪光灯未闪光
flash-forced-on = 强制开启
flash-forced-off = 强制关闭
flash-auto = 自动模式
flash-return-not-detected = 未检测到回闪
flash-return-detected = 检测到回闪
flash-red-eye = 防红眼

## 镜头

lens-focal-single = { $mm } mm
lens-focal-range = { $min }–{ $max } mm
lens-aperture-single = f/{ $f }
lens-aperture-range = f/{ $min }–{ $max }
lens-range = { $focal } { $aperture }

## 日期和时间

date-with-weekday = { $date } { $weekday }
date-time = { $date } { $time }
date-time-zone = { $datetime }（{ $zone }）
zone-utc = UTC
zone-offset = UTC{ $offset }
gps-date-time = { $date } { $time } UTC
detail-zone-from-gps = 时区由 GPS 时钟推算

## 位置

coordinates = { $latRef ->
    [south] 南纬
   *[north] 北纬
} { $lat }°，{ $lonRef ->
    [west] 西经
   *[east] 东经
} { $lon }°
altitude = { $ref ->
    [above] 海拔 { $m } m
    [below] 低于海平面 { $m } m
   *[sea] 位于海平面
}
compass-point = { $point ->
    [n] 北
    [nne] 北东北
    [ne] 东北
    [ene] 东东北
    [e] 东
    [ese] 东东南
    [se] 东南
    [sse] 南东南
    [s] 南
    [ssw] 南西南
    [sw] 西南
    [wsw] 西西南
    [w] 西
    [wnw] 西西北
    [nw] 西北
   *[nnw] 北西北
}
facing = 朝向{ $point }（{ $degrees }°）
detail-true-north = 相对于真北
detail-magnetic-north = 相对于磁北
accuracy = ±{ $m } m
distance-km = { $km } km
place = { $city }，{ $country }
place-city-only = { $city }
place-near = { $place }附近
place-detail = { $areas } · 距{ $city }{ $distance }
place-detail-distance = 距{ $city }{ $distance }

## 图像

dimensions = { $width } × { $height }
dimensions-megapixels = { $width } × { $height }（{ $mp } MP）
encoding-bit-depth = { $bits } 位
encoding-progressive = { $format }，渐进式
encoding-baseline = { $format }，基线
encoding-hierarchical = { $format }，分层
# machine-drafted, needs native review
encoding-alpha = { $format }，{ $alpha ->
    [premultiplied] 预乘 Alpha
    [transparency] 含透明色
   *[alpha] 含 Alpha
}
# [Lab], [Palette] and the variants after them: machine-drafted, needs native review
colour-model = { $model ->
    [YCbCr] YCbCr
    [RGB] RGB
    [Greyscale] 灰度
    [CMYK] CMYK
    [YCCK] YCCK
    [Lab] L*a*b*
    [Palette] 索引色
    [Multichannel] 多通道
    [Duotone] 双色调
    [YCgCo] YCgCo
    [XYB] XYB
    [YC] 亮度/色度 (YC)
    [Bayer] Bayer RAW
    [X-Trans] X-Trans RAW
    [CFA] 彩色滤镜阵列 RAW
    [LinearRaw] 线性 RAW
    [RGBE] RGBE (共享指数)
    [XYZE] XYZE (共享指数)
    [R] 单通道 (R)
    [RG] 双通道 (RG)
   *[other] { $model }
}
adobe-rgb-from-exif = Adobe RGB（来自 EXIF 标签；无嵌入配置文件）
orientation = { $value ->
    [1] 正常（无需旋转）
    [2] 水平翻转（已应用）
    [3] 旋转 180°（已应用）
    [4] 垂直翻转（已应用）
    [5] 水平翻转并逆时针旋转 90°（已应用）
    [6] 顺时针旋转 90°（已应用）
    [7] 水平翻转并顺时针旋转 90°（已应用）
    [8] 逆时针旋转 90°（已应用）
   *[other] 未知（{ $value }）
}
orientation-short = { $value ->
    [1] 正常
    [2] 水平翻转
    [3] 旋转 180°
    [4] 垂直翻转
    [5] 水平翻转并逆时针旋转 90°
    [6] 顺时针旋转 90°
    [7] 水平翻转并顺时针旋转 90°
    [8] 逆时针旋转 90°
   *[other] 值 { $value }
}

## 文件

file-size-bytes = { $num ->
   *[other] { $num } 字节
}
file-size-unit = { $num } { $unit ->
    [kb] KB
    [mb] MB
    [gb] GB
    [tb] TB
   *[pb] PB
}

## 版权

rating-rejected = 已拒绝
rating-stars = { $stars }（{ $n }/5）

## 内容凭证

c2pa-present = 存在（未验证）
c2pa-detail = 此文件包含 C2PA 清单，但其签名尚未经过验证。

## ICC 文件头值

icc-class = { $class ->
    [scnr] 输入设备
    [mntr] 显示设备
    [prtr] 输出设备
    [link] 设备链接
    [spac] 色彩空间
    [abst] 抽象
    [nmcl] 命名颜色
   *[other] 未知
}
icc-intent = { $intent ->
    [0] 可感知
    [1] 相对比色
    [2] 饱和度
    [3] 绝对比色
   *[other] 未知
}
icc-platform = { $platform ->
    [APPL] Apple
    [MSFT] Microsoft
    [SGI] Silicon Graphics
    [SUNW] Sun Microsystems
    [none] 未指定
   *[other] 未知
}
icc-coded = { $name }（{ $code }）

# machine-drafted, needs native review
label-modality = 模态
label-body-part = 身体部位
label-study = 检查
label-series = 序列
label-slice-thickness = 层厚
label-pixel-spacing = 像素间距
label-imager-pixel-spacing = 探测器像素间距
label-frames = 帧
label-transfer-syntax = 传输语法
label-document = 文档
label-document-type = 文档类型
medical-spacing = { $row } × { $column } mm
medical-mm = { $mm } mm
medical-lossy = { $name } (有损压缩)
detail-pixel-spacing = 患者体内像素中心之间的距离（行 × 列）。
# machine-drafted, needs native review
# Pixel spacing of an industrial inspection image (DICONDE, CT of an object).
detail-pixel-spacing-object = 物体内像素中心之间的距离（行 × 列）。
detail-imager-pixel-spacing = 在探测器处测得，未校正放大：患者体内的距离通常更小。

## 工业检测（DICONDE）

# machine-drafted, needs native review
label-nde-component-shape = 部件形状
label-nde-component-manufacturer = 部件制造商
label-nde-material = 材料
label-nde-thickness = 材料厚度
label-nde-pipe-diameter = 管径
label-nde-source = 射线源
label-nde-environment = 环境
label-nde-iqi = 像质计
label-nde-indications = 缺陷显示
label-nde-standard = 标准

## 研究用容积 (NIfTI、Analyze、NRRD、MetaImage)

# machine-drafted, needs native review
label-voxel-size = 体素大小
label-data-type = 数据类型
label-time-points = 时间点
label-orientation-source = 方向来源
label-intent = 数值含义
label-compression = 压缩
volume-voxel-size = { $x } × { $y } × { $z } mm
volume-orient-sform = NIfTI sform
volume-orient-qform = NIfTI qform
volume-orient-analyze = Analyze 文件头（不可靠）
volume-orient-space = NRRD 空间方向
volume-orient-transform = MetaImage 变换
volume-orient-spacings = 仅体素间距
volume-orient-none = 未说明
volume-orient-vox2ras = FreeSurfer vox2ras
volume-orient-mgh-default = FreeSurfer 默认值（不可靠）
volume-orient-minc = MINC 维度方向余弦
volume-orient-object = 物体坐标（无患者方向）
volume-content-projections = 投影（原始扫描）
volume-content-reconstruction = 重建体数据
volume-coords-rectilinear = 直线网格（间距不均，按均匀显示）
# Scan parameters and tags a FreeSurfer MGH file keeps after its voxels.
label-volume-tr = 重复时间
label-volume-te = 回波时间
label-volume-ti = 反转时间
label-volume-flip = 翻转角
label-volume-fov = 视野
label-volume-field = 场强
label-volume-pedir = 相位编码
label-volume-cmdline = 命令行
label-volume-xform = Talairach 变换
label-volume-content = 内容
label-volume-projections = 投影数
label-volume-angles = 旋转范围
label-volume-angle-step = 旋转步长
label-volume-kv = 管电压
label-volume-current = 管电流
label-volume-power = 射线源功率
label-volume-exposure = 曝光
label-volume-sod = 源到物体距离
label-volume-odd = 物体到探测器距离
label-volume-sdd = 源到探测器距离
label-volume-magnification = 光学放大倍数
label-volume-binning = 像素合并
label-volume-objective = 物镜
label-volume-filter = 射线源滤片
label-volume-camera = 探测器
label-volume-instrument = 仪器
label-volume-software = 软件
label-volume-date = 采集时间
label-volume-ring = 环形伪影校正
label-volume-smoothing = 平滑
label-volume-coords = 网格
# machine-drafted, needs native review
# An eye scan (OCT): which eye.
label-volume-eye = 眼别
volume-eye-right = 右眼 (OD)
volume-eye-left = 左眼 (OS)
detail-volume-unreliable = 文件未说明其坐标轴在患者体内的指向：左右可能颠倒，因此不显示方向标签。
# machine-drafted, needs native review
# An industrial CT volume: its axes are the scanner's, there is no patient.
detail-volume-object = 坐标轴属于扫描仪而非患者，因此不显示方向标签。

## 科学图像（FITS、XISF）

label-object = 天体
label-telescope = 望远镜
label-instrument = 仪器
label-filter = 滤光片
label-observed = 观测时间
label-exposure-time = 曝光时间
label-sample = 采样
label-unit = 单位
label-hdu = 扩展
label-centre = 中心
label-centre-deg = 中心（度）
label-frame = 坐标系
# machine-drafted, needs native review
science-frame-galactic = 银道
science-frame-ecliptic = 黄道
label-projection = 投影
label-pixel-scale = 像素比例
science-seconds = { $seconds } 秒
science-arcsec-px = 每像素 { $arcsec }″
label-distortion = 畸变
label-wcs = 天球坐标
science-wcs-unsupported = 不支持：{ $reason }
# machine-drafted, needs native review
science-wcs-approximate = 近似（仅指向，无畸变）
label-observer = 观测者
label-bayer = 拜耳阵列
label-capture-start = 拍摄开始 (UTC)
label-capture-end = 拍摄结束 (UTC)
label-frame-rate = 帧率
science-fps = { $fps } 帧/秒
science-frames-cut = { $frames } / { $declared }
# The HDF5 dataset or NetCDF variable shown.
label-dataset = 数据集
label-pixel-size = 像素大小
# "0.25 nm per pixel"; $size may be "0.25 × 0.5".
science-pixel-size = 每像素 { $size } { $unit }
# Microscopy (DigitalMicrograph, MRC, Gwyddion, Igor).
label-microscope = 显微镜
label-voltage = 加速电压
label-magnification = 放大倍数
label-imaging-mode = 模式
label-detector = 探测器
label-acquired = 采集时间
label-field-of-view = 视场
science-kilovolts = { $kv } kV
science-magnification = { $value }×
label-vendor = 厂商
label-scanner = 扫描仪
label-objective = 物镜
label-pyramid-levels = 层级
# machine-drafted, needs native review
label-emissivity = 发射率
label-object-distance = 目标距离
label-reflected-temperature = 反射温度
label-atmospheric-temperature = 大气温度
label-humidity = 相对湿度
label-raw-range = 原始值范围
label-temperature-range = 温度范围
thermal-temperature = { $value } { $unit }
thermal-metres = { $value } m
thermal-percent = { PERCENT($value) }
thermal-range = { $low } – { $high }
slide-levels = { $count }：{ $sizes }
# X-ray diffraction: the experiment geometry from a detector frame's header.
label-wavelength = 波长
# Ångström, e.g. "0.97625 Å".
xtal-angstrom = { $value } Å
label-photon-energy = 光子能量
xtal-kev = { $value } keV
# Sample to detector distance.
label-detector-distance = 探测器距离
xtal-mm = { $value } mm
# Where the direct beam meets the detector, in pixels from the image's top-left corner.
label-beam-centre = 光束中心
xtal-beam-centre = x { $x }, y { $y } px
# The detector arm's swing angle (2θ).
label-detector-swing = 探测器 2θ
xtal-degrees = { $value }°
# Goniometer angles (Greek letters are the field's usual names).
label-phi = φ 角
label-omega = ω 角
label-chi = χ 角
label-kappa = κ 角
# The sample's rotation during the exposure: start angle plus width, e.g. "12.5° + 0.1°".
label-oscillation = 振荡
xtal-oscillation = { $start }° + { $width }°
# $axis is the axis name from the file, e.g. "phi" or "omega".
xtal-oscillation-axis = 绕 { $axis } { $start }° + { $width }°
# The count at which a pixel saturates.
label-overload = 过载值
# The finest lattice spacing (d) recorded at the detector's edge and in its corners.
label-resolution = 分辨率
xtal-resolution = 边缘 { $edge } Å，角落 { $corner } Å
label-geometry = 几何
xtal-curved = 弯曲成像板：无 q、d 或分辨率环
xtal-incomplete = 头信息不完整：无 q、d 或分辨率环
# All-fields group title: "FITS header".
fields-science-header = { $format } 头
# machine-drafted, needs native review
fields-asdf-tree = ASDF 树
label-asdf-model = 数据模型
label-asdf-version = 文件版本
label-asdf-standard = ASDF 标准
label-asdf-library = 写入程序
# machine-drafted, needs native review
# All-fields group title for a CERN ROOT file's list of keys (objects).
fields-root-keys = ROOT 键
# CERN ROOT files: the ROOT version that wrote the file (e.g. "6.28/04"), its compression,
# the number of keys (stored objects), and the object shown: class (e.g. "TH2F"), its path in
# the file, title, number of entries and sum of weights.
label-root-version = ROOT 版本
label-root-compression = 压缩
label-root-keys = 键
label-root-class = 类
label-root-name = 名称
label-root-title = 标题
label-root-entries = 条目数
label-root-sum-weights = 权重和
# A graph's number of points.
label-root-points = 点数
# $axis is "x", "y" or "z".
label-root-axis = { $axis } 轴
# $quantity is the axis title with its unit, e.g. "Energy [GeV]"; $min and $max the axis
# limits.
root-axis-fixed = { $quantity }：{ $bins } 个 bin，{ $min } 至 { $max }
root-axis-variable = { $quantity }：{ $bins } 个可变宽度 bin，{ $min } 至 { $max }
label-root-mean-rms = 平均值和 RMS { $axis }
root-mean-rms = { $mean } ± { $rms }
# Trees (event data) are listed only, with their entry counts.
label-root-trees = 树
root-tree = { $name }（{ $entries } 个条目）
# machine-drafted, needs native review
# CASA images: what the image is (from its directory name), its restoring beam (major and
# minor axis, position angle), the beams of its planes when they differ, the rest frequency,
# spectral reference frame (e.g. "LSRK") and velocity definition of the frequency labels,
# the default mask (e.g. "mask0") and the casacore storage manager with its byte order.
label-casa-type = 图像类型
casa-type-image = 恢复图像
casa-type-residual = 残差
casa-type-model = 模型
casa-type-pb = 主波束
casa-type-mask = CLEAN 掩模
casa-type-psf = 点扩散函数
casa-type-sumwt = 权重之和
casa-type-weight = 权重
label-casa-beam = 恢复波束
casa-beam = { $major }″ × { $minor }″, 位置角 { $pa }°
label-casa-plane-beams = 各平面波束
casa-plane-beams = { $count } 个波束，长轴 { $min }″ 至 { $max }″
label-casa-rest-frequency = 静止频率
casa-ghz = { $ghz } GHz
label-casa-spectral-frame = 光谱参考系
label-casa-velocity = 速度定义
casa-velocity-radio = 射电
casa-velocity-optical = 光学
casa-velocity-relativistic = 相对论
label-casa-mask = 默认掩模
label-casa-storage = 存储
casa-storage-big = { $manager }，大端序
casa-storage-little = { $manager }，小端序
# machine-drafted, needs native review
# Unstructured meshes: the mesh's name in the file, the convention describing it (UGRID,
# ICON, MPAS), its counts, face shapes, coordinates and where the values sit.
label-ugrid-name = 网格
label-ugrid-convention = 约定
label-ugrid-nodes = 节点
label-ugrid-faces = 面
label-ugrid-edges = 边
label-ugrid-shapes = 面形状
label-ugrid-coordinates = 坐标
label-ugrid-values = 数值位于
label-ugrid-split = 为绘制而拆分
# $n is the formatted count, $count the number (for the plural).
ugrid-triangles = { $count ->
   *[other] { $count } 个三角形
}
ugrid-quads = { $count ->
   *[other] { $count } 个四边形
}
# Faces of five or more nodes; $max is the most nodes of one face.
ugrid-polygons = { $count ->
   *[other] { $count } 个多边形（最多 { $max } 个节点）
}
ugrid-lonlat = 经度和纬度
ugrid-projected = 投影 x 和 y
ugrid-projected-unit = 投影 x 和 y（{ $unit }）
ugrid-on-faces = 面（平涂）
ugrid-on-nodes = 节点（在面上插值）
# A mesh file without data variables: the image shows how many nodes each face has.
ugrid-grid-only = 无：图像显示每个面的节点数
# Faces cut at the antimeridian or closed to a pole so they draw on a flat map.
ugrid-split = { $count ->
   *[other] { $count } 个位于 180° 经线或极点的面
}
# machine-drafted, needs native review
# Slide annotations (NDP.view, Aperio ImageScope, ASAP sidecar files).
label-annotation-file = 文件
# "slide.xml (Aperio ImageScope)"
annotation-source = { $file }（{ $format }）
label-annotation-shapes = 形状
label-annotation-groups = 分组
# Shapes or points over the reader's limits, left out.
label-annotation-skipped = 已省略
annotation-polygon = 多边形
annotation-polyline = 折线
annotation-rectangle = 矩形
annotation-ellipse = 椭圆
annotation-point = 点
annotation-points = 点集
annotation-ruler = 标尺
annotation-arrow = 箭头
# The list of annotations is cut short: "and 1,200 more".
annotation-more = 以及另外 { $count } 个
# All-fields group title: "Annotations in slide.xml".
fields-annotations-header = { $file } 中的标注
label-drawing-version = 版本
label-drawing-units = 单位
label-drawing-extents = 范围
label-drawing-sheet = 图幅
label-drawing-layouts = 布局
label-drawing-layers = 图层
label-drawing-entities = 图元
label-drawing-codepage = 代码页
label-drawing-missing-fonts = 替换的字体
# machine-drafted, needs native review
# A font file the drawing names that was not found: the styles naming it, and the font drawing its text instead.
drawing-missing-font = { $file }（样式 { $styles }），以 { $substitute } 绘制
# A Big Font (double-byte text) that was not found; its characters come from other fonts.
drawing-missing-bigfont = { $file }（样式 { $styles }），以备用字体绘制
# Survey data (LandXML).
label-survey-crs = 坐标参考系
survey-crs-none = 未注明
label-survey-project = 项目名称
label-survey-application = 写入程序
label-survey-surface = 地形曲面
# A TIN surface: its name, counts, plan and sloped areas, the volume above its lowest point, its elevations and the contour interval drawn; unit is "m" or "ft" (areas in square units, volume in cubic units).
survey-surface = { $name }：{ $points } 个点，{ $triangles } 个三角形；平面面积 { $area } { $unit }²，表面积 { $area3d } { $unit }²；最低点以上体积 { $volume } { $unit }³；高程 { $zmin }–{ $zmax } { $unit }；等高距 { $interval } { $unit }
label-survey-alignment = 路线
# An alignment: its name, length and first and last stations ("1+234.50").
survey-alignment = { $name }：{ $length } { $unit }，桩号 { $start } 至 { $end }
# SHX shape fonts found and used for the drawing's text.
label-drawing-shx-fonts = SHX 字体
label-drawing-warnings = 问题
label-plot-paper = 纸张
# "2 of 5": the page shown and how many the plot has.
plot-page-of = { $page } / { $pages }
label-plot-page = 页
label-plot-pens = 使用的笔
# The printer language a plot came wrapped in, e.g. "PCL 5".
label-plot-job = 打印作业
# What a plot draws: strokes, fills and labels.
label-plot-items = 绘制项
# Width × height of what is drawn, in drawing units: "1,250 × 800 mm".
drawing-extents = { $width } × { $height } { $unit }
drawing-extents-unitless = { $width } × { $height }
drawing-model-space = 模型
# "24, 3 off or frozen"
drawing-layers-off = { $num }，其中 { $off } 个关闭或冻结
# DXF $INSUNITS.
drawing-units = { $code ->
    [1] 英寸
    [2] 英尺
    [3] 英里
    [4] 毫米
    [5] 厘米
    [6] 米
    [7] 千米
    [8] 微英寸
    [9] 密耳
    [10] 码
    [11] 埃
    [12] 纳米
    [13] 微米
    [14] 分米
    [15] 十米
    [16] 百米
    [17] 吉米
    [18] 天文单位
    [19] 光年
    [20] 秒差距
    [21] 美国测量英尺
   *[other] 无单位
}
# All-fields group title: "DXF entities".
fields-drawing-entities = { $format } 图元

## IC 版图 (GDSII)

# machine-drafted, needs native review
label-layout-library = 库
label-layout-units = 单位
# The database unit (the grid coordinates are stored in) and the user unit, with SI symbols:
# "1 nm database unit, 1 µm user unit".
layout-units = 数据库单位 { $db }，用户单位 { $user }
# The top cell shown (each top cell is a page).
label-layout-cell = 单元
label-layout-cells = 单元数
# Cells no other cell places.
label-layout-top-cells = 顶层单元
# "TOP, TEST, PAD and 12 more"
layout-top-cells-more = { $names } 及另外 { $more } 个
# Layer/datatype pairs.
label-layout-layers = 层
# Polygons, paths and boxes, as stored (each cell once).
label-layout-shapes = 图形
label-layout-texts = 标签
# Cell references and the placements they make: "1,204 (88,320 placements)".
label-layout-references = 引用
layout-references = { $refs } ({ $placements } { $placements ->
   *[other] 次放置
})
# Shapes with every cell placement expanded.
label-layout-flat-shapes = 展平后的图形
label-layout-bounds = 边界
# Corners of what the cell draws, micrometres: "-12.5, 0 to 2,900, 3,500 µm".
layout-bounds = { $x0 }, { $y0 } 至 { $x1 }, { $y1 } µm

## 电路板与原理图 (Gerber, Excellon, KiCad)

# machine-drafted, needs native review
group-board = 电路板
group-schematic = 原理图
# The board view shown (its layers, its top or bottom), or the schematic sheet.
label-pcb-view = 视图
label-pcb-project = 项目
label-pcb-revision = 版本
label-pcb-date = 日期
# The CAD program that wrote the file.
label-pcb-generator = 生成软件
# Width × height of the board outline: "100 × 80 mm".
label-pcb-size = 板尺寸
label-pcb-thickness = 板厚
label-pcb-copper-layers = 铜层
label-pcb-sheets = 图纸页
# The board's layers or files, each with its role: "board-F_Cu.gbr (Top copper)".
label-pcb-layers = 层
pcb-layer-role = { $name } ({ $role })
label-pcb-components = 封装
label-pcb-symbols = 符号
label-pcb-nets = 网络
label-pcb-tracks = 走线
label-pcb-wires = 导线
label-pcb-vias = 过孔
# Drilled holes and how many drill sizes: "1,204 in 9 sizes".
label-pcb-holes = 孔
pcb-holes = { $holes } 个,{ $sizes } { $sizes ->
   *[other] 种孔径
}
label-pcb-apertures = 光圈
# What a board layer or file is, shown after its name.
pcb-role-inner-copper = 内层铜 { $n }
pcb-role-top-copper = 顶层铜
pcb-role-bottom-copper = 底层铜
pcb-role-top-solder-mask = 顶层阻焊
pcb-role-bottom-solder-mask = 底层阻焊
pcb-role-top-silkscreen = 顶层丝印
pcb-role-bottom-silkscreen = 底层丝印
pcb-role-top-paste = 顶层钢网
pcb-role-bottom-paste = 底层钢网
pcb-role-top-solder-paste = 顶层锡膏
pcb-role-bottom-solder-paste = 底层锡膏
pcb-role-top-adhesive = 顶层胶
pcb-role-bottom-adhesive = 底层胶
pcb-role-top-courtyard = 顶层占位区
pcb-role-bottom-courtyard = 底层占位区
pcb-role-top-fabrication = 顶层装配
pcb-role-bottom-fabrication = 底层装配
pcb-role-board-outline = 板框
pcb-role-plated-drill = 金属化钻孔
pcb-role-non-plated-drill = 非金属化钻孔
pcb-role-drill = 钻孔
pcb-role-drill-holes = 钻孔孔位
pcb-role-mechanical = 机械层
pcb-role-margin = 边距
pcb-role-drawings = 图纸
pcb-role-comments = 注释
pcb-role-engineering-change-1 = 工程变更 1
pcb-role-engineering-change-2 = 工程变更 2
pcb-role-vias = 过孔
pcb-role-other = 其他
pcb-role-unknown = 未知
label-pcb-macros = 光圈宏

## 光谱 (JCAMP-DX, SPC, mzML, mzXML, Bruker NMR)

# machine-drafted, needs native review
group-spectrum = 光谱
label-spectrum-format = 格式
label-spectrum-technique = 技术
label-spectrum-title = 标题
# The page shown: "3 of 1,204: Scan 1532".
label-spectrum-page = 页
spectrum-page-of = { $page }/{ $pages }:{ $name }
# An axis with its unit: "Wavenumber (cm⁻¹)".
spectrum-axis = { $title } ({ $unit })
label-spectrum-x = X 轴
label-spectrum-y = Y 轴
label-spectrum-nucleus = 核
label-spectrum-frequency = 谱仪频率
label-spectrum-solvent = 溶剂
label-spectrum-pulse-program = 脉冲程序
label-spectrum-spectra = 光谱数
label-spectrum-points = 点数
label-spectrum-instrument = 仪器
# Who or what lab measured it.
label-spectrum-origin = 来源
label-spectrum-date = 测量时间
# How Specular made the page from the raw data (an FID transformed, phased automatically).
label-spectrum-processing = 处理
spectrum-technique-ir = 红外
spectrum-technique-raman = 拉曼
spectrum-technique-uv-vis = 紫外-可见
spectrum-technique-fluorescence = 荧光
spectrum-technique-nmr = 核磁共振
spectrum-technique-ms = 质谱
spectrum-technique-chromatogram = 色谱图
spectrum-technique-other = 光谱

## 3D 模型 (STL, OBJ, PLY, glTF, 3MF)

# machine-drafted, needs native review
group-mesh = 3D 模型
label-triangles = 三角形
label-vertices = 顶点
label-materials = 材质
label-textures = 纹理
label-up-axis = 向上轴
# Textures the model uses, some of which could not be read: "3 (1 missing)".
mesh-textures-missing = { $count }（缺失 { $missing }）

## 分子结构 (PDB, mmCIF, CIF, SDF, XYZ)

# machine-drafted, needs native review
group-structure = 结构
# The entry's identifier: a PDB ID such as 1CRN, or the CIF data block's name.
label-structure-id = 条目
label-structure-classification = 分类
# How the structure was determined: X-ray diffraction, electron microscopy, solution NMR…
label-structure-method = 方法
# The crystallographic or cryo-EM resolution, in ångströms.
label-structure-resolution = 分辨率
structure-resolution = { $value } Å
label-structure-deposited = 沉积
label-structure-formula = 化学式
label-structure-chains = 链
# Residues (amino acids, nucleotides) in the polymer chains.
label-structure-residues = 残基
label-structure-atoms = 原子
# Atoms after the unit cell was filled by symmetry, and those the file lists: "96 (12 in the asymmetric unit)".
structure-atoms-cell = { $count }（不对称单元中 { $asymmetric } 个）
label-structure-bonds = 键
# Models in the file (an NMR ensemble), of which the first is shown.
label-structure-models = 模型
# Molecules (SDF records) or data blocks in the file, of which the first is shown.
label-structure-records = 记录
structure-first-of = { $count }（显示第一个）
# Small molecules bound to the polymer, by their three-letter codes.
label-structure-ligands = 配体
# A list cut short: "HEM, NAG, SO4 and 9 more".
structure-ligands-more = { $list } 及另外 { $more } 个
label-structure-cell = 晶胞
# Unit cell edge lengths (Å) and angles (degrees).
structure-cell = a = { $a } Å, b = { $b } Å, c = { $c } Å, α = { $alpha }°, β = { $beta }°, γ = { $gamma }°
label-structure-space-group = 空间群
label-structure-secondary = 二级结构
structure-ss-file = 来自文件
structure-ss-computed = 由主链计算

## HDF4 和 HDF-EOS2

# machine-drafted, needs native review
group-hdf4 = HDF4
# The HDF library version that wrote the file, e.g. "NCSA HDF Version 4.2 Release 15".
label-hdf4-library = 库
# Counts of the file's scientific datasets, raster images, groups (Vgroups) and tables (Vdatas).
label-hdf4-datasets = 数据集
label-hdf4-images = 栅格图像
label-hdf4-vgroups = 组
label-hdf4-vdatas = 表
# HDF-EOS: NASA's Earth Observing System conventions on top of HDF4 (MODIS, ASTER, AIRS…).
group-hdf-eos = HDF-EOS
label-eos-version = 版本
# The grids, swaths and point structures the file defines.
label-eos-structures = 结构
eos-grids = { $count ->
   *[other] { $count }个网格（{ $names }）
}
eos-swaths = { $count ->
   *[other] { $count }个条带（{ $names }）
}
eos-points = { $count ->
   *[other] { $count }个点结构
}
# The grid or swath the dataset shown belongs to.
label-eos-structure = 结构
# The grid's map projection as its GCTP name, e.g. GCTP_SNSOID (sinusoidal).
label-eos-projection = 投影
label-eos-grid-size = 网格大小
# Columns × rows.
eos-grid-size = { $width } × { $height }
# The grid's upper-left and lower-right corners.
label-eos-corners = 角点
# The product's short name from the inventory metadata, e.g. MOD09A1.
label-eos-product = 产品
# When the data was acquired.
label-eos-time = 时间
eos-time = { $begin } 至 { $end }

## 行星图像（VICAR、PDS3、PDS4：NASA 和 ESA 任务数据）

# machine-drafted, needs native review
group-planetary = 行星图像
# The body observed, e.g. MARS, Ganymede.
label-planetary-target = 目标天体
label-planetary-mission = 任务
# The spacecraft, rover or lander carrying the instrument.
label-planetary-spacecraft = 航天器
label-planetary-start = 开始时间
label-planetary-stop = 结束时间
# The product's identifier in the archive.
label-planetary-product = 产品 ID
# The map projection of a map-projected image, as the label names it.
label-planetary-projection = 地图投影
label-planetary-resolution = 分辨率
planetary-metres-per-pixel = 每像素 { $metres } m
planetary-pixels-per-degree = 每度 { $pixels } 像素
# Latitude and longitude of the map projection's centre.
label-planetary-centre = 投影中心
planetary-lat-lon = { $lat }°, { $lon }°
# The latitudes and longitudes the map covers.
label-planetary-coverage = 覆盖范围
planetary-coverage = 纬度 { $south }° 至 { $north }°，经度 { $west }° 至 { $east }°
# The body's equatorial radius the map uses.
label-planetary-radius = 天体半径
planetary-km = { $km } km

## OMERO（从 OMERO 服务器或 Image Data Resource 打开的图像）

# machine-drafted, needs native review
group-omero = OMERO
label-omero-server = 服务器
# $server the server's address; $user the user name signed in with.
omero-server-user = { $server }（以 { $user } 身份）
omero-server-public = { $server }（公开）
label-omero-image = 图像
# $name the image's name; $id its number on the server.
omero-image = { $name }（ID { $id }）
# The project and dataset the image is in, as "Project › Dataset".
label-omero-project = 项目
# The screen, plate and well, as "Screen › Plate › B4".
label-omero-screen = 筛选
label-omero-owner = 所有者
# $owner the image's owner; $group the OMERO group it belongs to.
omero-owner-group = { $owner }（{ $group }）
label-omero-acquired = 采集时间
label-omero-dimensions = 尺寸
omero-size = { $width } × { $height }
omero-z = { $count ->
   *[other] { $count } 个 Z 平面
}
omero-c = { $count ->
   *[other] { $count } 个通道
}
omero-t = { $count ->
   *[other] { $count } 个时间点
}
# The physical size of a pixel along X, Y (and Z).
label-omero-physical = 像素大小
label-omero-channels = 通道
# A channel not shown in the server's rendering.
omero-channel-hidden = { $channel }（已隐藏）
# How the pixels are fetched from the server: rendered tiles (JPEG) or the whole plane (PNG).
label-omero-fetched = 获取方式
omero-fetched-tiles = { $width } × { $height } 的 { $encoding } 图块，{ $levels ->
   *[other] { $levels } 级
}
omero-fetched-whole = 整个平面，格式 { $encoding }

## 反射变换成像 (PTM, RTI, Relight)

# machine-drafted, needs native review
group-rti = 反射变换
# The reflectance model the coefficients belong to: PTM (polynomial), HSH (hemispherical harmonics), RBF…
label-rti-basis = 基
label-rti-order = 阶数
# Coefficients per colour channel.
label-rti-terms = 系数
# How colour is stored: LRGB (one colour, varying luminance), RGB, YCC…
label-rti-colour = 颜色布局
# Coefficient images stored in the file.
label-rti-planes = 平面
# The byte-to-coefficient mapping the file states, per coefficient.
label-rti-scale = 缩放
label-rti-bias = 偏置

## 点云

# machine-drafted, needs native review
group-points = 点云
label-points = 点数
# Points read of those in the file, when a very large cloud was evenly thinned: "32,000,000 of 120,000,000".
points-subsampled = { $loaded } / { $total }
# The LAS point data record format number (0–10).
label-point-format = 点格式
# The coordinate reference system.
label-crs = 坐标参考系
# What each point carries besides its position.
label-attributes = 属性
points-channel-rgb = RGB
points-channel-intensity = 强度
points-channel-classification = 分类
points-channel-returns = 回波
# ASPRS classes present, with point counts: "Ground (1,234), Building (567)".
label-classes = 类别
# E57 scans, with point counts.
label-scans = 扫描
point-class-0 = 从未分类
point-class-1 = 未分类
point-class-2 = 地面
point-class-3 = 低矮植被
point-class-4 = 中等植被
point-class-5 = 高植被
point-class-6 = 建筑物
point-class-7 = 低点（噪声）
point-class-8 = 模型关键点
point-class-9 = 水体
point-class-10 = 铁路
point-class-11 = 路面
point-class-12 = 重叠
point-class-13 = 地线
point-class-14 = 导线
point-class-15 = 输电塔
point-class-16 = 导线连接器
point-class-17 = 桥面
point-class-18 = 高噪声
point-class-19 = 架空结构
point-class-20 = 忽略的地面
point-class-21 = 雪
point-class-22 = 时间排除
# A class without a standard name, by number.
point-class-other = 类别 { $class }

## Gaussian splats

# machine-drafted, needs native review
group-splats = 高斯泼溅
label-splats = 泼溅点
# Splats shown of those in the file, when a very large file was evenly thinned: "12,000,000 of 30,000,000".
splats-shown-of = { $shown } / { $total }
# The degree (1 to 3) of the spherical harmonics that give each splat its view-dependent colour.
label-splat-harmonics = 视角相关颜色
splats-harmonics-none = 无
# $format: the file format, e.g. "SPZ".
splats-format-antialiased = { $format }，使用抗锯齿训练

## 警告

# machine-drafted, needs native review
warn-not-diagnostic = 不可用于诊断。Specular 是通用图像查看器，不是医疗器械：其显示未针对诊断进行校准。

warn-gps = 此文件记录了拍摄地点。如需保密，请在分享前移除位置信息。
warn-null-island = GPS 位置恰好为 0°, 0°；相机可能未能定位，因此该位置很可能有误。
warn-date-future = 拍摄日期在未来；相机时钟可能设置有误。
warn-icc-damaged = 嵌入的颜色配置文件已损坏，无法读取。
warn-metadata-failed = 由于 Specular 出错，无法读取元数据。图像本身完好。
warn-orientation-conflict = EXIF 与 XMP 的方向信息不一致（EXIF：{ $exif }，XMP：{ $xmp }）。将使用 EXIF 值。
warn-exif-damaged = 部分 EXIF 数据无法读取（{ $error }）；其余数据已显示。
warn-xmp-damaged = XMP 元数据格式有误（{ $error }）；仅能读取其中一部分。

## 关于对话框

geonames-attribution = 地名：GeoNames（www.geonames.org），依据知识共享署名 4.0 许可协议（CC BY 4.0，https://creativecommons.org/licenses/by/4.0/）授权。离线查询由 Grant Miner 开发的 reverse_geocoder crate 提供（MIT 或 Apache-2.0）。数据已转换为城市列表，按“原样”提供，不附带任何担保。

## TIFF and DNG structure values (All fields)
# machine-drafted, needs native review

subfile-type = { $value ->
    [0] 全分辨率图像
    [1] 降低分辨率的图像
    [2] 多页图像中的一页
    [4] 透明蒙版
   *[other] 其他
}
predictor = { $value ->
    [1] 无
    [2] 水平差分
    [3] 浮点差分
   *[other] 其他
}
extra-samples = { $value ->
    [0] 未指定
    [1] 预乘 Alpha
    [2] 直通 Alpha
   *[other] 其他
}
sample-format = { $value ->
    [1] 无符号整数
    [2] 有符号整数
    [3] 浮点
    [4] 未定义
   *[other] 其他
}

## 地理空间数据 (GeoTIFF, GeoJSON, Shapefile, GeoPackage, KML/KMZ, GPX)

# machine-drafted, needs native review
group-geospatial = 地理空间
label-geo-crs = 坐标参考系
label-geo-datum = 基准面
label-geo-projection = 投影
label-geo-unit = 单位
label-geo-bounds = 范围
label-geo-lonlat-bounds = 经纬度
label-geo-pixel-size = 像素大小
label-geo-overviews = 概览
label-geo-nodata = NoData 值
label-geo-raster-type = 栅格类型
label-geo-vertical = 垂直基准
label-geo-citation = 引用说明
label-geo-features = 要素
label-geo-vertices = 顶点
label-geo-layer = 图层
label-geo-layers = 图层
label-geo-tiles = 瓦片
label-geo-encoding = 文本编码
label-geo-warnings = 问题
# The companion files a georeference was read from ("scene.jgw, scene.prj").
label-geo-sidecars = 附属文件
# machine-drafted, needs native review
# A projection Specular cannot convert to longitude and latitude: $method is its name.
geo-projection-unsupported = { $method }（不支持坐标）
# How longitude and latitude are brought to WGS 84 (what GPS and web maps use).
label-geo-to-wgs84 = 转换到 WGS 84
geo-to-wgs84-same = 无需转换
geo-to-wgs84-helmert = 基准面转换（Helmert）
geo-to-wgs84-by-area = 基准面转换（Helmert，按区域选择）
geo-to-wgs84-unknown = 未知：坐标使用文件自身的基准面
# A coordinate system's name and its EPSG code: "WGS 84 / UTM zone 33N (EPSG:32633)".
geo-crs-epsg = { $name } (EPSG:{ $code })
geo-crs-unknown = 未注明
# Map bounds: the lower-left and upper-right corners.
geo-bounds = { $x0 }, { $y0 } 至 { $x1 }, { $y1 }
# Bounds in degrees: west to east longitude, south to north latitude.
geo-lonlat-bounds = 经度 { $west }° 至 { $east }°，纬度 { $south }° 至 { $north }°
geo-pixel-size = { $x } × { $y }
geo-overviews = { $n ->
   *[other] { $n } 级概览：{ $sizes }
}
geo-pixel-is-point = 像素为点
geo-layer = { $features ->
   *[other] { $features } 个要素
}，{ $fields ->
   *[other] { $fields } 个字段
}
# A GeoPackage tile pyramid: its table, the zoom level shown and its tile count.
geo-tiles = { $table }，缩放级别 { $zoom }，{ $tiles ->
   *[other] { $tiles } 个瓦片
}
fields-geo-geokeys = GeoTIFF 键
fields-geo-gdal = GDAL 元数据
fields-geo-wkt = 坐标参考系（WKT）
fields-geo-projjson = 坐标参考系（PROJJSON）
# machine-drafted, needs native review
# What a web map tile set (MBTiles, PMTiles) says about itself: container, tile type, zoom
# levels, description and other metadata.
fields-geo-tileset = 图块集
# The credit line a map's data asks to be shown with it ("© OpenStreetMap contributors").
label-geo-attribution = 署名
# The header lines of a georeferenced raster format (ENVI, ASCII grid, DTED, ERDAS IMAGINE).
fields-geo-header = 文件头
# machine-drafted, needs native review
# GIFTI files: the format version, what the file holds, the number of data arrays and the
# encoding of their values (e.g. "GZipBase64Binary").
label-gifti-version = GIFTI 版本
label-gifti-role = 内容
gifti-role-surface = 表面
gifti-role-labels = 每顶点标签
gifti-role-data = 每顶点数值
label-gifti-arrays = 数据数组
label-gifti-encoding = 编码
label-gifti-vertices = 顶点
label-gifti-triangles = 三角形
# The brain structure, e.g. "CortexLeft".
label-gifti-structure = 结构
# A surface's geometric type, e.g. "Anatomical", "Inflated".
label-gifti-geometry = 几何类型
# The coordinate transform of the vertices: $from and $to are coordinate spaces such as
# "scanner", "Talairach" or "MNI 152".
label-gifti-transform = 坐标
gifti-transform = { $from }（无需变换到 { $to }）
gifti-transform-applied = { $from } → { $to }（已应用）
# The surface file a data file is drawn on, found beside it.
label-gifti-surface = 显示于
# The data array shown: its name, NIfTI intent (e.g. "NIFTI_INTENT_SHAPE") and data type.
label-gifti-array = 数组
label-gifti-intent = Intent
label-gifti-data-type = 数据类型
# Entries of the label table (names and colours of regions).
label-gifti-labels = 标签表
# CIFTI-2 files: the version, NIfTI intent (e.g. "ConnDenseScalar") and matrix size.
label-cifti-version = CIFTI 版本
label-cifti-intent = Intent
label-cifti-matrix = 矩阵
cifti-matrix = { $rows } × { $columns }
# The maps (pages) of the file and what they are.
label-cifti-maps = 图
cifti-maps-scalars = { $count } 个标量图
cifti-maps-labels = { $count } 个标签图
cifti-maps-series = { $count } 个时间点
cifti-maps-parcels = { $count } 个分区
cifti-maps-other = { $count }
# machine-drafted, needs native review
# A dense connectivity file (.dconn.nii): every brain place's connectivity to every other.
# $seeds: how many places can be picked; $size: the bytes read when one is picked (e.g. "357 KB").
label-cifti-dense = 连接
cifti-dense = { $seeds } 个种子点，每个种子点读取 { $size }
# A time series' start and step; $unit is e.g. "second".
label-cifti-series = 序列
cifti-series = 从 { $start } 起，每 { $step }（{ $unit }）
label-cifti-models = 脑模型
label-cifti-parcels = 分区
label-cifti-structures = 结构
label-cifti-voxels = 皮层下体素
# A surface the file's values lie on: the brain structure (e.g. "CORTEX_LEFT"), its vertex
# count, and the surface file found beside the file for it, or that none was found.
label-cifti-surface = 表面
cifti-surface-found = { $structure }：{ $vertices } 个顶点，{ $file }
cifti-surface-missing = { $structure }：需要含 { $vertices } 个顶点的表面（文件旁没有）
# How the file is shown: on the surfaces found beside it, or as its subcortical voxels.
label-cifti-shown = 显示为
cifti-shown-surfaces = 着色表面
cifti-shown-voxels = 皮层下体素的体积
# machine-drafted, needs native review
# Shown with a picked seed's row: click a surface vertex, or move the crosshair in MPR.
cifti-shown-connectivity = 所选种子点的连接
# Maps left out of the voxel volume, which would be too large with them.
label-cifti-left-out = 省略的图
# Tractograms: format, number of streamlines (fibre tracts) and points.
label-tract-format = 格式
label-tract-streamlines = 流线
# A large tractogram draws only some of its streamlines.
tract-streamlines-shown = { $count }（显示 { $shown } 条）
label-tract-points = 点
label-tract-mean-length = 平均长度
# A length in millimetres.
tract-mm = { $value } mm
# Values per point and per streamline the file stores, by name.
label-tract-scalars = 每点标量
label-tract-properties = 每条流线属性
# TrackVis: the voxel grid the tracts were made in, its voxel size and voxel order (e.g. "LAS").
label-tract-grid = 网格
label-tract-voxel-size = 体素大小
label-tract-voxel-order = 体素顺序
# How the points were brought into scanner (RAS) millimetres.
label-tract-space = 坐标
tract-space-vox-to-ras = 来自体素到 RAS 矩阵
tract-space-voxel-order = 来自体素顺序（无体素到 RAS 矩阵）
tract-space-scanner = 扫描仪空间
# MRtrix: the tracking step size.
label-tract-step = 步长

## File headers (DPX, Cineon, XWD, MIFF, TIM, WSQ, JBIG)

# machine-drafted, needs native review
# What the file's own header records, for formats without Exif or XMP.
group-header = 文件头
label-header-created = 创建时间
# The program or person that wrote the file.
label-header-creator = 创作者
label-header-copyright = 版权
label-header-project = 项目
label-header-comment = 注释
# The scanner, camera or other device the image came from.
label-header-device = 设备
# SMPTE time code of the frame: "01:00:00:12".
label-header-timecode = 时间码
# A film frame's edge code (KeyKode).
label-header-edge-code = 边缘码
label-header-frame-rate = 帧率
# Where the frame sits in its sequence.
label-header-frame = 帧
# Film stock and slate.
label-header-film = 胶片
# How code values relate to light: printing density, linear, BT.709…
label-header-transfer = 传递函数
# The colour primaries the file states.
label-header-colorimetric = 色度
# The X window a screen dump was taken of.
label-header-window = 窗口
label-header-resolution = 分辨率
label-header-compression = 压缩
# machine-drafted, needs native review
# JPEG XS: the profile and the level the codestream states ("Main 422.10"; "4k-1, sublevel 6 bpp").
label-header-profile = 档次
# machine-drafted, needs native review
label-header-level = 级别
# machine-drafted, needs native review
# JPEG XS: how many wavelet levels, horizontal and vertical ("5 horizontal, 1 vertical").
label-header-decomposition = 小波分解级数
# machine-drafted, needs native review
# JPEG XS: the colour transform ("None", "Reversible colour transform (RCT)", "Star-Tetrix, RGGB mosaic").
label-header-colour-transform = 颜色变换
# machine-drafted, needs native review
# Photoshop: the artboards, by name and size ("Phone (375 × 667), Tablet (768 × 1024)").
label-header-artboards = 画板
# machine-drafted, needs native review
# Photoshop file without a merged image, shown from its layers: the features it leaves out,
# by Photoshop's names ("Color Lookup, Knockout").
label-header-not-drawn = 未绘制
# machine-drafted, needs native review
# The same, for the features drawn only approximately ("Bevel & Emboss, Satin").
label-header-approximated = 近似绘制

## Fonts (TTF, OTF, TTC/OTC, WOFF, WOFF2)

# machine-drafted, needs native review
group-font = 字体
label-font-family = 字族
label-font-style = 样式
# The face's place in a font collection: "2 of 4".
label-font-face = 字面
font-face-of = 第 { $index } 个，共 { $count } 个
label-font-version = 版本
label-font-designer = 设计师
# The font's maker (the name table's manufacturer).
label-font-manufacturer = 字体厂商
label-font-glyphs = 字形
label-font-outlines = 轮廓
font-outlines-truetype = TrueType（二次）
font-outlines-cff = PostScript（CFF，三次）
font-outlines-cff2 = PostScript（CFF2，三次）
font-outlines-bitmap = 仅位图
font-outlines-none = 无
# What the font's licence flags (OS/2 fsType) allow documents that embed it to do.
label-font-embedding = 嵌入
font-embedding-installable = 可嵌入并安装。
font-embedding-restricted = 不可嵌入。
font-embedding-preview-print = 仅可嵌入用于查看和打印。
font-embedding-editable = 可嵌入可编辑的文档。
font-embedding-no-subsetting = 只能整体嵌入，不能取子集。
font-embedding-bitmap-only = 只能作为位图。
# Variation axes of a variable font: "Weight 100–900 (400)".
label-font-axes = 可变轴
font-axis = { $name } { $min }–{ $max }（{ $default }）
label-font-instances = 命名样式
label-font-colour = 彩色字形
font-colour-colr0 = COLR 图层
font-colour-colr1 = COLR v1 渐变
font-colour-sbix = 位图（sbix）
font-colour-cbdt = 位图（CBDT）
font-colour-svg = SVG
label-font-weight = 字重
label-font-units-per-em = 每 em 单位数
label-font-tables = 表
label-font-licence = 许可证
label-font-licence-url = 许可证网址
label-font-copyright = 版权
label-font-trademark = 商标

## Office documents (Word, Excel, PowerPoint, iWork, OpenDocument)

# machine-drafted, needs native review
group-document = 文档
label-document-kind = 类型
document-kind-word = Word 文档
document-kind-excel = Excel 工作簿
document-kind-powerpoint = PowerPoint 演示文稿
document-kind-pages = Pages 文稿
document-kind-numbers = Numbers 表格
document-kind-keynote = Keynote 演示文稿
document-kind-odf-text = OpenDocument 文本
document-kind-odf-spreadsheet = OpenDocument 电子表格
document-kind-odf-presentation = OpenDocument 演示文稿
document-kind-odf-graphics = OpenDocument 绘图
document-kind-odf-other = OpenDocument 文件
# machine-drafted, needs native review
document-kind-playlist = 播放列表
# machine-drafted, needs native review
label-playlist-items = 项目
# Appended to the kind: "Word document, template, with macros".
document-template = 模板
document-macros = 含宏
document-legacy = Office 97–2003 格式
label-document-preview = 预览
# The image format of the preview stored in the file, e.g. "JPEG image".
document-preview-image = { $format } 图像
document-preview-none = 此文件中没有
label-document-subject = 主题
label-document-author = 作者
label-document-description = 描述
label-document-category = 类别
label-document-last-saved-by = 最后保存者
label-document-created = 创建时间
label-document-modified = 修改时间
label-document-printed = 最后打印时间
label-document-pages = 页数
label-document-slides = 幻灯片数
label-document-hidden-slides = 隐藏幻灯片数
label-document-notes = 备注页数
label-document-sheets = 工作表数
label-document-words = 字数
label-document-characters = 字符数
label-document-paragraphs = 段落数
label-document-lines = 行数
label-document-editing-time = 编辑时间
document-minutes = { $minutes } 分钟
label-document-revision = 修订号
label-document-application = 保存程序
label-document-company = 公司
label-document-template = 模板

## Books (EPUB)

# machine-drafted, needs native review
group-book = 图书
label-book-authors = 作者
label-book-publisher = 出版者
label-book-date = 出版日期
label-book-language = 语言
label-book-pages = 页数
# Entries in the book's table of contents.
label-book-contents = 目录条目
label-book-layout = 版式
book-layout-reflowable = 可重排
book-layout-fixed = 固定版式
book-right-to-left = 从右到左
label-book-version = 格式
book-version = EPUB { $version }
label-book-fonts = 嵌入的字体
book-fonts-obfuscated = { $count }（{ $obfuscated } 个已混淆）
label-book-subjects = 主题
label-book-identifier = 标识符
label-book-rights = 权利

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

# machine-drafted, needs native review
volume-orient-parrec = Philips 角度与偏心
volume-orient-ecat = ECAT 体素尺寸与偏移（不可靠）
label-volume-technique = 技术
label-volume-version = 格式版本
label-volume-isotope = 同位素
label-volume-tracer = 示踪剂
label-volume-units = 单位

# Text, tables and subtitles.
label-header-encoding = 字符集
label-header-lines = 行数
label-header-line-ends = 换行符
label-header-words = 字数
label-header-pages = 页数
label-header-pictures = 图片
label-header-pictures-not-shown = 未显示的图片
label-header-tables = 表格
label-header-rows = 表格行数
label-header-columns = 列数
label-header-delimiter = 分隔符
label-header-column-names = 列名
label-header-cues = 字幕
label-header-cue-span = 字幕起止
