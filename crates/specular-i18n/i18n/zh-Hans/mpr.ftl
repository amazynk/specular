# Multiplanar reformatting (MPR).

palette-mpr = 多平面重建 (MPR)
palette-mpr-slab = 下一 MPR 层块（薄层、MIP、MinIP、平均）
disabled-no-volume = 这不是容积：请打开序列、多帧文件、3D 容积或 Z 堆栈
disabled-mpr-closed = 请先打开多平面视图
mpr-axial = 轴位
mpr-coronal = 冠状位
mpr-sagittal = 矢状位
mpr-oblique = 斜位
mpr-xy = XY
mpr-xz = XZ
mpr-yz = YZ
mpr-3d = 3D 视图
mpr-thin = 薄层
mpr-mip = MIP
mpr-minip = MinIP
mpr-average = 平均
mpr-slab = 层块：{ $mode }，{ $thickness } mm
# machine-drafted, needs native review
mpr-slab-angstrom = 层块：{ $mode }，{ $thickness } Å
mpr-slab-unit = 层块：{ $mode }，{ $thickness } { $unit }
mpr-slab-voxels = 层块：{ $mode }，{ $thickness } 体素
mpr-thicker = 加厚层块
mpr-thinner = 减薄层块
mpr-reset = 重置视图
mpr-close = 关闭 MPR
mpr-maximise = 最大化视图
mpr-export-planes = 导出平面
# machine-drafted, needs native review
# Streamlines (TrackVis .trk, MRtrix .tck) found beside a brain volume, drawn over each
# view within a slab about its plane; Alt+S steps the slab, then hides them. $files: the
# files' names; $thickness: the slab in millimetres.
mpr-tracts = { $files } 的流线：层块 { $thickness } mm
mpr-tracts-off = { $files } 的流线：已隐藏
mpr-tracts-none = 此体积旁没有 TrackVis 或 MRtrix 流线
mpr-position = { $view } { $position } mm
# machine-drafted, needs native review
mpr-position-angstrom = { $view } { $position } Å
mpr-position-unit = { $view } { $position } { $unit }
mpr-position-voxels = { $view } { $position }
mpr-length-unit = { $value } { $unit }
mpr-length-voxels = { $value } 体素
mpr-axis-label = { $axis }：{ $label }
mpr-axis-position = { $axis }：{ $position }/{ $count }
mpr-axis-channel = 通道
mpr-axis-unnamed = 轴 { $axis }
mpr-one-channel = MPR 一次只显示一个通道
# machine-drafted, needs native review
# $count: channels drawn together in their colours.
mpr-composite = { $count } 个通道以彩色显示，各用其显示范围
# $reason: error-reason-needs-memory ("it needs 6 GB of free RAM, but only 4 GB is free right now. ...").
mpr-composite-memory = 只显示一个通道：{ $reason }
mpr-isotropic = 文件中没有体素大小：体素按立方体显示
mpr-isotropic-z = 文件中没有层间距：层面按相隔一个像素显示
# A seismic volume's inline and crossline sections (the time slice reads "Time … ms").
mpr-seismic-inline = 主测线 { $inline }
mpr-seismic-crossline = 联络测线 { $crossline }
mpr-value = { $value } { $unit }
mpr-value-plain = { $value }
mpr-loading = 正在加载 { $loaded } / { $count } 个层面
mpr-loaded = { $count } 个层面，{ $width } × { $height }
mpr-downsampled = 以 1/{ $factor } 分辨率显示（内存或图形限制）
mpr-converted = 数值以 32 位浮点显示
mpr-opening = 正在打开容积…
mpr-gpu-memory = 显卡上 { $size }
mpr-refused = 无法构建容积：{ $reason }
mpr-failed = 容积未加载：{ $reason }
mpr-no-gpu = 显卡无法容纳此容积：{ $reason }
mpr-planes-saved = 已将 { $count } 个平面保存到 { $folder }
mpr-planes-failed = 无法保存平面：{ $reason }
mpr-view-a11y = { $view } 视图，{ $position } mm
mpr-crosshair-a11y = 十字线位于 { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
mpr-crosshair-a11y-angstrom = 十字线位于 { $x }, { $y }, { $z } Å
mpr-crosshair-a11y-unit = 十字线位于 { $x }, { $y }, { $z } { $unit }
mpr-crosshair-a11y-voxels = 十字线位于体素 { $x }, { $y }, { $z }
mpr-controls = MPR 控件

# 3D view palette entries.
palette-mpr-preset = 下一 3D 预设（CT 骨骼、软组织、肺、血管造影；MR）
palette-mpr-render = 下一 3D 渲染（明暗着色、Cinematic、MIP、MinIP）
palette-mpr-clip = 下一 3D 裁剪（无、2D 视图的方框、切除一角）
palette-mpr-export-stl = 将 3D 表面导出为 STL…
# machine-drafted, needs native review
palette-mpr-tract-slab = MPR 中的下一流线层块
palette-mpr-maximise = 最大化 MPR 视图，或显示所有视图
palette-mpr-export-planes = 将 MPR 平面导出为图像…

# The 3D view: transfer function presets, renderings, clipping; STL surface export.
vr-preset = { $preset ->
    [bone] CT 骨骼
    [soft] CT 软组织
    [lung] CT 肺
    [angio] CT 血管造影
   *[mr] MR
}
vr-mode = { $mode ->
    [cinematic] Cinematic
    [mip] MIP
    [minip] MinIP
   *[shaded] 明暗着色
}
vr-clip = { $clip ->
    [box] 裁剪：视图方框
    [cut] 裁剪：切除一角
   *[off] 不裁剪
}
vr-smooth = 平滑表面
vr-export = 导出 STL
vr-threshold = 表面阈值 { $value }
vr-stl-exporting = 正在构建表面…
vr-stl-saved = { $count ->
    [0] 此阈值下没有内容：未保存表面
   *[other] 已将 { $count } 个三角形保存到 { $name }
}
vr-stl-failed = 无法保存表面：{ $reason }
vr-view-a11y = { $preset }，{ $mode }，{ $clip }，表面阈值 { $threshold }。Shift+方向键环绕，加号和减号缩放。

## 原始体数据：无文件头的 .raw / .vol 文件的数据布局

# machine-drafted, needs native review
raw-volume-title = 打开原始体数据
raw-volume-file = { $file } 没有文件头。请说明其样本的排列方式。
raw-volume-guess-label = 建议的布局
raw-volume-guess = 第 { $index } 个，共 { $count } 个
raw-volume-custom = 自定义
raw-volume-width = 宽度（体素）
raw-volume-height = 高度（体素）
raw-volume-slices = 层面数
raw-volume-type = 样本类型
raw-volume-type-unsigned = 无符号 { $bits } 位
raw-volume-type-signed = 有符号 { $bits } 位
raw-volume-type-float = 浮点 { $bits } 位
raw-volume-order = 字节序
raw-volume-little = 小端序
raw-volume-big = 大端序
raw-volume-offset = 文件头偏移
raw-volume-bytes = { $count ->
   *[other] { $count } 字节
}
raw-volume-pixel = 像素大小
raw-volume-spacing = 层间距
raw-volume-mm = { $value } mm
raw-volume-exact = 恰好填满文件（{ $size }）
raw-volume-fits = 使用 { $file } 中的 { $size }，剩余 { $left }
raw-volume-too-big = 需要 { $size }，但文件只有 { $file }
raw-volume-cancel = 取消
raw-volume-open = 打开
# machine-drafted, needs native review
# A dense connectivity file (CIFTI .dconn.nii) over voxels: the volume shown is one seed's
# row; letting go of the crosshair on another seed's voxel shows that one. $seed: the
# structure as the file names it and the voxel, such as "THALAMUS_LEFT (4, 6, 3)".
mpr-dconn-seed = { $seed } 的连接；将十字线移到另一个种子点
# machine-drafted, needs native review
mpr-measure-hint-edit = 拖动端点可移动；拖到另一端点上即可删除
# machine-drafted, needs native review
mpr-measure-removed = 已删除测量
mpr-measure-lengths-only = MPR 仅测量长度
mpr-more = 更多…

## Curved MPR: a reformat along a curve clicked on a plane.
## machine-drafted, needs native review

palette-mpr-curve = 沿点击的曲线进行曲面 MPR
palette-mpr-curve-layout = 曲面 MPR：拉直或拉伸
palette-mpr-curve-save = 保存曲线…
palette-mpr-curve-load = 加载曲线…
palette-mpr-export-segments = 将每个分割段导出为 STL…
palette-mpr-dental = 牙科布局（全景和横断面）
palette-mpr-nerve = 描记下颌管
cpr-kind = { $kind ->
    [stretched] 拉伸
   *[straightened] 拉直
}
cpr-button = 曲面
cpr-controls = 曲面 MPR 控件
cpr-at = 位于 { $position }
cpr-section = 横断面
cpr-hint-start = 在 { $view } 视图中沿结构点击各点
cpr-hint-more = 再点击一个点
cpr-add-points = 添加点
cpr-undo-point = 撤销点
# machine-drafted, needs native review
cpr-mark = 在此标记
cpr-mark-set = 已设置标记 { $n }。
cpr-mark-removed = 已移除标记。
cpr-mark-here = 标记 { $n }
cpr-diameter = 直径 { $value }
cpr-diameters = 直径 { $min } 至 { $max }
cpr-mark-span = 标记 { $from } 至 { $to }：沿中心线 { $length }
cpr-mark-total = 全部标记：{ $length }
cpr-clear = 清除曲线
cpr-save = 保存曲线
cpr-load = 加载曲线
cpr-done = 完成
cpr-a11y = { $name }，长度 { $length }，光标位于 { $position }。滚轮移动光标。
cpr-none = 尚无曲线：请先点击各点
cpr-cleared = 已清除曲线
cpr-saved = 已保存曲线：{ $name }
cpr-save-failed = 无法保存曲线：{ $reason }
cpr-loaded = 已加载曲线
# machine-drafted, needs native review
# $count: centrelines read from the file.
cpr-loaded-count = { $count ->
   *[other] 已加载 { $count } 条中心线
}
# $count: centrelines found in files saved beside the volume.
cpr-sidecars-found = { $count ->
   *[other] 找到 { $count } 条已保存的中心线
}
# A centreline without a name, by its place in the list.
cpr-centreline = 中心线 { $number }
cpr-new = 新建中心线
cpr-rename = 重命名
cpr-name = 中心线名称
cpr-delete = 删除
# $name: the centreline's name.
cpr-delete-title = 要删除中心线“{ $name }”吗？
cpr-delete-detail = 其点将被移除。磁盘上已保存的文件会保留。
cpr-deleted = 已删除“{ $name }”
cpr-load-failed = 无法加载曲线：{ $reason }
cpr-load-not-json = 该文件不是 JSON
cpr-load-not-curves = 该文件不是 Specular 曲线
cpr-load-empty = 该文件中没有包含两个或更多点的曲线
cpr-panoramic = 全景
cpr-panoramic-slab = 全景，{ $thickness } mm 层块
cpr-panoramic-thin = 全景，薄层

## The dental layout (Medical).
## machine-drafted, needs native review

dental-button = 牙科
dental-not-for-diagnosis = 不可用于诊断或治疗计划
dental-nerve = 下颌管
dental-nerve-hint = 在全景或横断面上沿下颌管点击各点
dental-nerve-done = 已关闭下颌管描记
dental-spacing = 每 { $spacing } mm
dental-section = 断面 { $number }
dental-off = 请先打开牙科布局
dental-suggesting = 正在查找牙弓…
dental-suggested = 已建议牙弓：拖动其点进行调整
dental-no-arch = 未找到牙弓：请在轴位视图中沿牙弓点击各点
dental-vendor = { $vendor } 牙科 CBCT

## Each segment as its own STL.
## machine-drafted, needs native review

seg-stl-export-button = 导出分割段
seg-stl-detail = 分割段：{ $count } 个三角形
seg-stl-detail-full = 分割段：完整细节
seg-stl-none = 没有可导出的分割段
seg-stl-not-labels = 此体积不是标签图
seg-stl-exporting = 正在构建各分割段的表面…
seg-stl-saved = 已将 { $count } 个分割段保存到 { $folder }
seg-stl-open = 未闭合：{ $count }。
seg-stl-empty = 分割段为空：未保存任何内容
seg-stl-failed = 无法保存分割段：{ $reason }
# machine-drafted, needs native review
toolbar-mpr = 三平面 (MPR)
toolbar-mpr-close = 关闭三平面 (MPR)
settings-viewing-toolbar-items-mpr = 三平面 (MPR)
