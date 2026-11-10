# machine-drafted, needs native review
# Specular — 病理：染色分离、标注导出与导入、叠加在容积上的标签容积。简体中文。

## 命令面板
palette-stain-separation = 分离染色
palette-stain-1 = 显示或隐藏染色 1
palette-stain-2 = 显示或隐藏染色 2
palette-stain-3 = 显示或隐藏染色 3
palette-stain-mode = 以透光或光密度显示染色
palette-stain-estimate = 从区域估计染色
palette-stain-normalise = 标准化染色颜色
palette-stain-reference = 将区域用作染色参考
palette-stain-reset = 已发表的染色向量
palette-export-annotations = 导出标注…
palette-import-annotations = 导入标注…
palette-label-volume = 叠加标签容积…

## 不可用原因
disabled-not-colour-image = 需要彩色图像
disabled-stains-off = 请先开启染色分离
disabled-no-region = 请先绘制区域
disabled-no-slide-annotations = 没有可导出的标注、区域或点
disabled-no-label-target = 请在多平面视图中打开容积，或打开医学图像

## 染色名称
stain-hematoxylin = 苏木精
stain-eosin = 伊红
stain-dab = DAB
stain-residual = 残余
stain-set-he = H&E
stain-set-hdab = H-DAB
stain-set-hedab = H&E-DAB

## 染色提示与图例
stain-off = 染色分离已关闭
# { $set }：H&E、H-DAB 或 H&E-DAB。
stain-on = 染色：{ $set }
stain-shown = 已显示{ $stain }
stain-hidden = 已隐藏{ $stain }
stain-mode-light = 以透光显示染色
stain-mode-od = 以光密度显示染色
# 图例标题：“H&E · 透光”“H-DAB · 光密度”。
stain-legend-light = { $set } · 透光
stain-legend-od = { $set } · 光密度
stain-legend-estimated = 从区域估计
stain-legend-normalised = 已标准化
stain-legend-published = 已发表的向量
# { $pixels }：使用的染色像素数。
stain-estimated = 已从 { $pixels } 个像素估计染色
stain-estimate-failed = 无法估计染色：{ $reason }
stain-estimate-background = 区域大部分是背景
stain-estimate-one-colour = 区域只有一种颜色
stain-estimate-three = 估计需要 H&E 或 H-DAB
stain-estimate-no-pixels = 区域的像素尚未加载
stain-reference-set = 已将区域设为染色参考
stain-normalise-on = 染色颜色已标准化
stain-normalise-off = 染色颜色保持扫描原样
stain-reset-done = 已发表的染色向量

## 标注导出对话框
annot-export-title = 导出标注
annot-export-format = 格式
annot-export-format-coco = COCO JSON
annot-export-format-geojson = GeoJSON (QuPath)
annot-export-format-masks = 标签掩膜
annot-export-output = 掩膜保存为
annot-export-output-png = PNG 图块
annot-export-output-tiff = TIFF 图块
annot-export-output-ome = OME-TIFF 金字塔
annot-export-region = 范围
annot-export-region-slide = 整张切片
annot-export-region-view = 可见区域
annot-export-region-drawn = 最后绘制的区域
annot-export-level = 层级
# { $level }：0 为全分辨率；{ $width } × { $height } 为其像素尺寸。
annot-export-level-value = { $level } · { $width } × { $height }
annot-export-tile = 图块大小
annot-export-empty = 空图块
annot-export-empty-skip = 跳过
annot-export-empty-keep = 写入
annot-export-what = { $count ->
   *[other] { $count } 个形状
}，{ $classes ->
   *[other] { $classes } 个类别
}
annot-export-lines = 线条不会导出到 COCO 和掩膜中。
annot-export-export = 导出…
annot-export-writing = 正在写入…
# { $files }：写入的文件数。
annot-export-done = { $files ->
   *[other] 已导出 { $files } 个文件
}
annot-export-failed = 导出失败：{ $reason }
annot-import-done = { $count ->
   *[other] 已导入 { $count } 个标注
}
annot-import-failed = 无法导入：{ $reason }
annot-import-skipped = { $count ->
   *[other] { $count } 个形状没有可用的轮廓
}
annot-file-coco = COCO JSON
annot-file-geojson = GeoJSON

## 标签容积
label-volume-file = 标签容积
label-volume-table = 颜色表
# { $file }：标签容积的文件名；{ $count }：其标签数。
label-volume-loaded = { $count ->
   *[other] { $file }：{ $count } 个标签
}
label-volume-failed = 无法叠加 { $file }：{ $reason }
label-volume-outside = { $file } 与此容积不重叠
label-volume-not-whole = 其值不是整数
label-volume-colour = 它包含颜色而非标签
label-volume-empty = 它没有标签
label-volume-too-many = 它的标签过多
label-volume-too-large = 它太大，无法重采样
label-volume-degenerate = 它的方向无效
# 颜色表中没有名称的标签。
label-volume-unnamed = 标签 { $value }
