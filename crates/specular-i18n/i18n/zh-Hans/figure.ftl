# Figures, key images and teaching files.

## Command palette
# machine-drafted, needs native review
palette-export-figure = 导出图件…
# machine-drafted, needs native review
palette-export-figure-help = 将当前视图保存为带比例尺、色条和标注的图件。
# machine-drafted, needs native review
palette-toggle-key-image = 标记为关键图像
# machine-drafted, needs native review
palette-toggle-key-image-help = 将此图像标记为关键图像，或取消标记。
# machine-drafted, needs native review
palette-copy-key-images = 复制关键图像
# machine-drafted, needs native review
palette-copy-key-images-help = 将关键图像复制为一张图片。
# machine-drafted, needs native review
palette-export-key-images = 导出关键图像…
# machine-drafted, needs native review
palette-export-key-images-help = 将关键图像保存为幻灯片或 PNG 文件夹。
# machine-drafted, needs native review
palette-export-teaching-file = 导出教学文件…
# machine-drafted, needs native review
palette-export-teaching-file-help = 保存不含患者信息的关键图像，用于教学。

## The figure dialog
# machine-drafted, needs native review
figure-dialog-title = 导出图件
# machine-drafted, needs native review
figure-format = 格式
# machine-drafted, needs native review
figure-format-png = PNG，16 位
# machine-drafted, needs native review
figure-format-tiff = TIFF，16 位
# machine-drafted, needs native review
figure-region = 区域
# machine-drafted, needs native review
figure-region-visible = 可见区域
# machine-drafted, needs native review
figure-region-whole = 整幅图像
# machine-drafted, needs native review
figure-size = 大小
# machine-drafted, needs native review
figure-size-pixels = 图像像素 (1:1)
# $width is a width in pixels.
# machine-drafted, needs native review
figure-size-width = 宽 { $width } px
# machine-drafted, needs native review
figure-scale-bar = 比例尺
# machine-drafted, needs native review
figure-scale-bar-none = 无校准
# machine-drafted, needs native review
figure-colour-bar = 色条
# machine-drafted, needs native review
figure-colour-bar-none = 无颜色映射或窗宽窗位
# machine-drafted, needs native review
figure-inset = 插图
# machine-drafted, needs native review
figure-inset-rectangle = 上一个矩形
# machine-drafted, needs native review
figure-inset-none = 请先绘制一个矩形
# machine-drafted, needs native review
figure-channels = 通道
# machine-drafted, needs native review
figure-channels-merged = 合并
# machine-drafted, needs native review
figure-channels-split = 分开并合并
# machine-drafted, needs native review
figure-channels-one = 单个通道
# machine-drafted, needs native review
figure-measurements = 测量
# $count is the number of measurements.
# machine-drafted, needs native review
figure-measurements-on = { $count ->
   *[other] 开 ({ $count })
}
# machine-drafted, needs native review
figure-measurements-none = 无
# machine-drafted, needs native review
figure-title-row = 标题
# machine-drafted, needs native review
figure-provenance = 来源
# machine-drafted, needs native review
figure-provenance-metadata = 在文件中
# machine-drafted, needs native review
figure-provenance-drawn = 在文件中并绘制
# machine-drafted, needs native review
figure-file = 文件
# machine-drafted, needs native review
figure-choose = 选择…
# machine-drafted, needs native review
figure-on = 开
# machine-drafted, needs native review
figure-off = 关
# $source is where the value comes from (a calibration, a colour map).
# machine-drafted, needs native review
figure-on-from = 开 · { $source }
# machine-drafted, needs native review
figure-export = 导出
# machine-drafted, needs native review
figure-working = 正在导出…
# machine-drafted, needs native review
figure-not-full-res = 未加载全分辨率像素。请缩放到 { PERCENT(100) }，或选择一个大小。
# machine-drafted, needs native review
figure-channel-red = 红
# machine-drafted, needs native review
figure-channel-green = 绿
# machine-drafted, needs native review
figure-channel-blue = 蓝
# $map is a colour map's name; $lo and $hi the values at its ends; $unit their unit.
# machine-drafted, needs native review
figure-legend-map = { $map }，{ $lo } 至 { $hi } { $unit }
# $lo and $hi are the window's ends; $unit their unit (HU).
# machine-drafted, needs native review
figure-legend-window = 窗口 { $lo } 至 { $hi } { $unit }
# $preset is a channel-colour preset's id.
# machine-drafted, needs native review
figure-display-channels = 通道颜色 { $preset }
# $channel is the channel shown (red, green, blue, alpha, luma).
# machine-drafted, needs native review
figure-display-channel = 通道 { $channel }
# machine-drafted, needs native review
figure-display-hilo = HiLo 标记
# machine-drafted, needs native review
figure-display-as-stored = 按存储值
# $value is the calibration (source and spacing).
# machine-drafted, needs native review
figure-caption-calibration = 校准：{ $value }
# $value is the display settings.
# machine-drafted, needs native review
figure-caption-display = 显示：{ $value }
# machine-drafted, needs native review
figure-not-image = 图件由查看器中的图像生成。
# machine-drafted, needs native review
figure-not-ready = 图像仍在加载。
# $width and $height are in pixels.
# machine-drafted, needs native review
figure-too-large = 太大：{ $width } × { $height } px。请选择可见区域或一个大小。

## Key images
# machine-drafted, needs native review
figure-keys-title = 导出关键图像
# machine-drafted, needs native review
figure-teaching-title = 导出教学文件
# machine-drafted, needs native review
figure-keys-images = 图像
# $count is the number of key images.
# machine-drafted, needs native review
figure-keys-count = { $count ->
   *[other] { $count } 张关键图像
}
# machine-drafted, needs native review
figure-keys-pptx = 幻灯片 (PPTX)
# machine-drafted, needs native review
figure-keys-folder = PNG 文件夹
# machine-drafted, needs native review
figure-keys-preset = 预设
# machine-drafted, needs native review
figure-keys-standard = 标准
# machine-drafted, needs native review
figure-keys-teaching = 教学文件
# machine-drafted, needs native review
figure-keys-overlays-on = 已绘制，并写入备注
# machine-drafted, needs native review
figure-keys-masks = 内嵌文字
# machine-drafted, needs native review
figure-keys-masks-auto = 已涂黑
# machine-drafted, needs native review
figure-keys-notes = 病例备注
# machine-drafted, needs native review
figure-keys-destination = 保存到
# machine-drafted, needs native review
figure-keys-empty = 还没有关键图像。请先将图像标记为关键图像。
# machine-drafted, needs native review
figure-keys-teaching-note = 患者信息将被省略，内嵌文字将被涂黑。分享前请检查每张图像。
# machine-drafted, needs native review
figure-keys-none-left = 没有可在不含患者信息的情况下导出的图像。
# machine-drafted, needs native review
figure-keys-copied = 关键图像已复制
# $count is how many key images there are now.
# machine-drafted, needs native review
figure-key-marked = { $count ->
   *[other] 已标记为关键图像 ({ $count })
}
# $count is how many key images are left.
# machine-drafted, needs native review
figure-key-unmarked = { $count ->
   *[other] 不再是关键图像 (剩余 { $count })
}
# $n is a frame or page number.
# machine-drafted, needs native review
figure-key-frame = 图像 { $n }
# $n is the key image's number in the set.
# machine-drafted, needs native review
figure-key-n = 关键图像 { $n }

## Results
# $name is the file or folder written.
# machine-drafted, needs native review
figure-saved = 已保存 { $name }
# $name is the file or folder written; $count how many images were left out.
# machine-drafted, needs native review
figure-saved-skipped = { $count ->
   *[other] 已保存 { $name }。有 { $count } 张图像被略过：找不到其内嵌文字。
}
# $reason says why.
# machine-drafted, needs native review
figure-failed = 无法导出：{ $reason }
# $name is a file that is already in the folder.
# machine-drafted, needs native review
figure-folder-taken = { $name } 已在该文件夹中。请选择一个空文件夹。

## The viewer
# machine-drafted, needs native review
figure-key-badge = 关键图像
