# DICOM 结构化报告、波形、放疗剂量和眼科断层成像。 machine-drafted, needs native review

## 结构化报告内容树 (报告模式下的标签浏览器)
sr-tree-title = { $title } ({ $kind })
sr-col-item = 条目
sr-col-concept = 概念
sr-col-type = 类型
sr-col-relationship = 关系
sr-col-value = 值
sr-show-image = 显示图像
sr-jump-missing = 引用的图像不在此文件夹中
sr-jump-frame = 此参考坐标系中没有包含该点的图像

## 图表 (剂量体积直方图、心电图纸)
plot-caliper-time = { $ms } ms · { $rate } 次/分
plot-caliper-time-zero = 0 ms
plot-caliper-hint = 在图表上拖动以测量
plot-adjust-hint = + 和 − 调整增益，[ 和 ] 调整走纸速度

## 心电图纸
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
   *[other] 另有 { $count } 组通道未显示
}

## MPR 上的放疗剂量 (等剂量线、颜色映射、图例、读数)
dose-value = 剂量 { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = 剂量已隐藏 (按 Shift+U 显示)
dose-on-rx = 剂量：处方 { $dose } { $unit } · 最大 { $max } { $unit }
dose-on-max = 剂量：占最大值的百分比，{ $max } { $unit }
dose-colour = 剂量以 { $colormap } 显示，{ PERCENT($blend) }
dose-dvh-hint = { $count ->
   *[other] Shift+K：{ $count } 个结构的剂量体积直方图
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = 剂量显示为等剂量线
dose-mode-colour = 剂量显示为颜色映射

## 剂量体积直方图 (图表页)
dvh-title = 剂量体积直方图
dvh-col-roi = 结构
dvh-col-volume = 体积
dvh-axis-dose = 剂量 ({ $unit })
dvh-axis-volume = 体积 (%)
dvh-prescription = 处方剂量 { $dose } Gy · 累积
dvh-max = 最大剂量 { $dose } { $unit } · 累积
dvh-none = 此参考坐标系没有可用于统计剂量直方图的结构集

## 眼科断层成像 (B 扫描滑块；角落里的定位图像)
oct-bscan = B 扫描
oct-depth = 深度 { $depth } µm
