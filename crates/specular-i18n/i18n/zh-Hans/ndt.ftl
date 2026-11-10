# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = 检测
ndt-probe = 探针：{ $name }
ndt-probe-profile = 线剖面
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = 双丝像质计 (IQI)
ndt-probe-wire-iqi = 单丝像质计 (IQI)
ndt-probe-crack = 裂缝宽度
ndt-probe-density = 密度点

ndt-hint-profile = 拖出一条线，绘制沿线的灰度值。[ 和 ] 切换探针。
ndt-hint-snr = 在均匀区域拖出一个区域。区域至少扩大到 20 × 55 像素。
ndt-hint-duplex-iqi = 沿像质计拖出一条线，穿过所有丝对。
ndt-hint-wire-iqi = 横跨能看到的最细的丝拖出一条短线。
ndt-hint-crack = 沿裂缝单击，然后按 Enter。在 10 个点读取宽度。
ndt-hint-density = 单击一个点读取其灰度值。输入其已知密度进行校准。

ndt-refused-needs-scale = 请先设置比例：丝径需要以毫米计。请在已知长度上使用“校准”。
ndt-refused-nothing-found = 沿这条线没有高出噪声的信号。
ndt-refused-too-small = 图像小于 20 × 55 像素的 SNR 区域。
ndt-refused-no-noise = 此区域没有可测量的噪声（所有像素都相同）。

ndt-reading = 正在读取…
ndt-no-pixels = 无法读取像素。

ndt-profile-range = 灰度 { $min } 至 { $max }
ndt-profile-mean = 平均灰度 { $mean }
ndt-profile-density = 密度 { $min } 至 { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value }（SRb { $srb } µm）
ndt-snrn-needs-srb = SNRn 需要 SRb：请测量双丝像质计或直接输入。
ndt-snr-stats = 平均值 { $mean }，σ { $sd }，区域 { $w } × { $h } px

ndt-duplex-result = 第一个未分辨丝对 { $pair }：SRb { $srb } µm，不清晰度 { $unsharpness }
ndt-duplex-all-resolved = 找到的丝对均已分辨。
ndt-duplex-dips = 谷值：{ $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = 插值：iSRb { $isrb } µm，不清晰度 { $unsharpness }

ndt-wire-result = 丝号 { $wire }，{ $diameter }
ndt-wire-measured = 测得宽度 { $width }（FWHM），直径约 { $diameter }

ndt-crack-mean = 平均宽度 { $width }
ndt-crack-range = 最窄 { $min }，最宽 { $max }
ndt-crack-cuts = 测量点数：{ $count }

ndt-density-value = 灰度 { $grey }，密度 { $density }
ndt-density-grey = 灰度 { $grey }
ndt-density-uncalibrated = 尚未进行密度校准：请在两个点输入已知密度。

ndt-ask-density = 此处已知密度：{ $value }（Enter 应用，留空按 Enter 跳过）
ndt-ask-srb = 基本空间分辨率 SRb：{ $value } µm（Enter 应用，Esc 跳过）
ndt-bad-density = 请输入 0 到 10 之间的密度。
ndt-bad-srb = 请以微米为单位输入 SRb，例如 80。
ndt-srb-set = 此图像的 SRb 已设为 { $srb } µm。
ndt-density-one-more = 已保留密度点。请再在一个点输入密度。
ndt-density-calibrated = 已根据两个点校准密度。
ndt-sidecar-failed = 无法将检测设置保存在文件旁边：{ $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = 相对起飞点 { $metres } m
map-export-kml = 将照片保存为 KMZ 或 KML…
map-export-kmz-full = 将照片保存为含完整照片的 KMZ…
map-kml-heading = 航向 { $degrees }°
map-kml-height = 相对起飞点 { $metres } m
map-kml-nothing = 没有带位置的照片可保存。
map-kml-saved = 已将 { $count } 张照片保存到 { $name }。
map-kml-failed = 无法保存地图文件：{ $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = 检测报告
inspection-report-default-title = 检测报告
inspection-report-folder = 添加此文件夹中的其他照片
inspection-report-docx = 同时保存 Word 文档 (DOCX)
inspection-report-kmz = 同时保存照片位置的 KMZ
inspection-report-caption = 带测量的照片
inspection-report-col-value = 值
inspection-report-col-scale = 比例来源
inspection-report-docx-failed = 无法写入 Word 文档。
inspection-report-unreadable = 无法读取任何照片。
