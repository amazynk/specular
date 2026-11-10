# Specular — Develop export. 简体中文.
# Exports are new files; the photo and its edit are never changed.

# machine-drafted, needs native review
palette-develop-export = 导出…
# machine-drafted, needs native review
palette-develop-export-help = 将编辑后的照片（一张或多张）写入为新文件。

## The dialog

# machine-drafted, needs native review
develop-export-title = 导出
# machine-drafted, needs native review
develop-export-start = 导出
# machine-drafted, needs native review
develop-export-preset = 预设
# machine-drafted, needs native review
develop-export-preset-custom = 自定义
# machine-drafted, needs native review
develop-export-preset-jpeg = JPEG，完整尺寸
# machine-drafted, needs native review
develop-export-preset-web = 网页，2048 像素
# machine-drafted, needs native review
develop-export-preset-tiff16 = 用于打印的 16 位 TIFF
# machine-drafted, needs native review
develop-export-preset-exr = OpenEXR，线性
# machine-drafted, needs native review
develop-export-save-preset = 保存预设
# machine-drafted, needs native review
develop-export-preset-saved = 已保存为预设“{ $name }”。
# machine-drafted, needs native review
develop-export-preset-exists = 这已是预设“{ $name }”。
# machine-drafted, needs native review
develop-export-delete-preset = 删除预设
# machine-drafted, needs native review
develop-export-delete-preset-ask = 删除“{ $name }”？
# machine-drafted, needs native review
develop-export-delete-preset-confirm = 删除预设“{ $name }”？再按一次 Delete 即可删除。已导出的文件会保留。
# machine-drafted, needs native review
develop-export-preset-deleted = 已删除预设“{ $name }”。已导出的文件会保留。
# machine-drafted, needs native review
develop-export-preset-name = { $format } · { $space } · { $size }
# machine-drafted, needs native review
develop-export-size-full = 完整尺寸
# machine-drafted, needs native review
develop-export-format-exr = OpenEXR
# machine-drafted, needs native review
develop-export-depth-half = 16 位浮点
# machine-drafted, needs native review
develop-export-depth-float = 32 位浮点
# machine-drafted, needs native review
develop-export-space = 色彩空间
# machine-drafted, needs native review
develop-export-space-srgb = sRGB
# machine-drafted, needs native review
develop-export-space-display-p3 = Display P3
# machine-drafted, needs native review
develop-export-space-rec2020 = Rec. 2020
# machine-drafted, needs native review
develop-export-space-adobe-rgb = Adobe RGB（兼容）
# machine-drafted, needs native review
# HDR output of an export. PQ and HLG are HDR files; a gain map is an SDR photo that also shows in HDR where the screen can.
develop-export-hdr = HDR
# machine-drafted, needs native review
develop-export-hdr-off = 关（SDR）
# machine-drafted, needs native review
develop-export-hdr-pq = PQ
# machine-drafted, needs native review
develop-export-hdr-hlg = HLG
# machine-drafted, needs native review
develop-export-hdr-gain-map = 增益图

## Progress and the end

# machine-drafted, needs native review
develop-export-progress = 正在导出第 { $done } 张，共 { $total } 张
# machine-drafted, needs native review
develop-export-progress-eta = 正在导出第 { $done } 张，共 { $total } 张 · 剩余约 { $time }
# machine-drafted, needs native review
develop-export-done = { $count ->
   *[other] 已导出 { $count } 张图像
}
# machine-drafted, needs native review
develop-export-stopped = 已在 { $count } 张后停止
# machine-drafted, needs native review
develop-export-running = 已有导出或转换正在进行。
# machine-drafted, needs native review
develop-export-row = 导出
# machine-drafted, needs native review
develop-export-row-detail = { $done } / { $total }
# machine-drafted, needs native review
develop-export-starting = 正在开始
# machine-drafted, needs native review
develop-export-no-gpu = 导出需要图形设备，但无法打开任何设备。
