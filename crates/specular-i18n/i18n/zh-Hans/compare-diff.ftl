# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = 与…比较
palette-compare-with-help = 将此图像与你选择的另一张图像比较。
palette-compare-side = 并排比较
palette-compare-side-help = 将比较的图像并排显示。
palette-compare-split = 分割比较
palette-compare-split-help = 将第二张图像显示在第一张之上，在可拖动的分割线处切开。
palette-compare-split-direction = 旋转分割线
palette-compare-split-direction-help = 将分割线由竖向改为横向，或改回。
palette-compare-overlay = 叠加比较
palette-compare-overlay-help = 将第二张图像混合叠加在第一张之上。
palette-compare-difference = 差异比较
palette-compare-difference-help = 显示图像的不同之处；再按切换到下一种。
palette-compare-flicker = 在比较的图像之间闪烁
palette-compare-flicker-help = 立即显示另一张图像。
palette-compare-auto-flicker = 自动闪烁
palette-compare-auto-flicker-help = 自动在两张图像之间切换，或停止。
palette-compare-align = 对齐比较的图像
palette-compare-align-help = 对齐不同尺寸的图像：左上、居中、缩放或自动查找。
palette-compare-metric-space = 切换测量空间
palette-compare-metric-space-help = 在线性光或编码 sRGB 中测量。
palette-compare-whole-image = 测量整张图像
palette-compare-whole-image-help = 测量整张图像而不是视图内的部分，或改回。
palette-compare-probe = 比较像素探针
palette-compare-probe-help = 显示指针下两张图像的值。
palette-compare-link = 联动或解除联动比较的图像
palette-compare-link-help = 分别缩放和平移两张图像，或重新一起操作。
palette-compare-close = 关闭比较
palette-compare-close-help = 返回图库。
disabled-compare-closed = 请先打开比较
disabled-compare-not-two = 适用于两张图像

## Cheat sheet
cheatsheet-compare-split = 分割或并排
cheatsheet-compare-split-direction = 分割线横向或竖向
cheatsheet-compare-auto-flicker = 自动闪烁
cheatsheet-compare-align = 下一种对齐
cheatsheet-compare-space = 测量空间
cheatsheet-compare-whole = 整张图像或视图内
cheatsheet-compare-probe = 像素探针
cheatsheet-compare-link = 联动或解除联动两张图像

## Settings
settings-gallery-compare-metric-space = 比较的测量空间
settings-gallery-compare-metric-space-help = 比较在何处测量 MAE、MSE、PSNR 和 SSIM。线性光即屏幕显示的效果。sRGB 在 8-bit 文件上与其他工具一致。
settings-gallery-compare-metric-space-linear = 线性光
settings-gallery-compare-metric-space-srgb = 编码 sRGB
settings-gallery-compare-flicker-limit = 最快自动闪烁
settings-gallery-compare-flicker-limit-help = 每秒最多切换的图像次数。超过 3 次，闪光可能诱发癫痫发作。

## Toolbar and captions
compare-split = 分割
compare-split-down = 分割线竖向
compare-split-across = 分割线横向
compare-link = 已联动
compare-auto-flicker = 自动
compare-flicker-rate = 每秒 { $rate } 次切换
compare-gain = 增益 ×{ $gain }
compare-probe = 探针
compare-kind-signed = 有符号误差
compare-kind-on-image = 叠在图像上的误差
compare-align-top-left = 左上
compare-align-centre = 居中
compare-align-scaled = B 缩放到 A
compare-align-auto = 自动对齐
compare-space-linear = 线性光
compare-space-srgb = sRGB
compare-space-help-linear = 在线性光中测量，与色调映射后的显示一致。此处的 PSNR 不是常见的 8-bit sRGB PSNR。
compare-space-help-srgb = 在编码 sRGB 值上测量，与其他工具处理 8-bit 文件的方式相同。高于 SDR 白的 HDR 按白计算。
compare-space-help-common = ΔE2000 始终使用 CIELAB (D65)，SDR 白为 L* 100，HDR 被裁切。大于 { $size } 像素的图像按 { $size } 像素的副本测量。
compare-region-view = 视图内
compare-region-whole = 整张图像
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM 需要 11 像素
compare-delta-e = ΔE { $mean }（{ PERCENT(95) }：{ $p95 }，最大 { $max }）
compare-delta-e-hdr = ΔE 已裁切 HDR
compare-aligned-centre = 居中，比较了 { $width } × { $height }
compare-aligned-scaled = B 缩放 { $scale }×，比较了 { $width } × { $height }
compare-aligned-auto = 已对齐 { $angle }° { $scale }×，比较了 { $width } × { $height }
compare-aligning = 正在对齐…
compare-align-failed = 无法对齐这些图像。已恢复为左上。
compare-split-label = { $b } 叠在 { $a } 上，分割线位于 { PERCENT($percent) }
compare-showing-which = 正在显示 { $which }：{ $name }
compare-auto-flicker-on = 正在闪烁，每秒 { $rate } 次切换
compare-auto-flicker-off = 闪烁已停止
compare-linked = 图像已联动
compare-unlinked = 图像已解除联动：各自平移
compare-with-title = 比较对象
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A：{ $a }。B：{ $b }。ΔE2000 { $delta }。
compare-a11y-divider = 分割线
compare-a11y-space = 测量空间
compare-a11y-align = 对齐
