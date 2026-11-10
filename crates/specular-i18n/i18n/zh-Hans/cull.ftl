# machine-drafted, needs native review
# Specular — 筛选模式 (DESIGN.md §6.3).

palette-cull = 进入或退出筛选模式
cheatsheet-cull = 筛选模式
settings-section-cull = 筛选模式
settings-cull-auto-enter = 筛选 RAW 文件夹
settings-cull-auto-enter-help = 以筛选模式直接打开主要包含 RAW 文件的文件夹。
settings-cull-auto-advance = 标记后转到下一张
settings-cull-auto-advance-help = 选用、拒绝或星级评分后前进。颜色标签和取消标记会停留在当前图像。
settings-cull-show = RAW+JPEG 配对时显示
settings-cull-show-help = 同名的 RAW 和 JPEG 作为一张图像筛选，标记同时应用于两者。JPEG 打开更快。
settings-cull-show-jpeg = JPEG
settings-cull-show-raw = RAW

# machine-drafted, needs native review
settings-cull-bursts = 堆叠连拍
settings-cull-bursts-help = 同一相机快速连续拍摄的照片显示为一个堆叠。S 打开或关闭堆叠，B 保留当前图像并拒绝堆叠中的其余图像。
settings-cull-burst-gap = 连拍间隔
settings-cull-burst-gap-help = 同一连拍中两张照片之间的最长间隔。
settings-cull-watch = 显示新文件
settings-cull-watch-help = 筛选时文件夹中出现的文件（连接的相机、导入）在完整写入后添加到末尾。
settings-cull-follow-newest = 跟随最新文件
settings-cull-follow-newest-help = 每个新文件到达时跳转到它。筛选时按 T 切换。
settings-cull-slot-1 = 文件夹 1
settings-cull-slot-1-help = Alt+1 将图像（连同其 RAW 或 JPEG 及附属文件）复制到此文件夹，Alt+Shift+1 将其移动到此处。
settings-cull-slot-1-unset = 未设置
settings-cull-slot-2 = 文件夹 2
settings-cull-slot-2-help = Alt+2 将图像（连同其 RAW 或 JPEG 及附属文件）复制到此文件夹，Alt+Shift+2 将其移动到此处。
settings-cull-slot-2-unset = 未设置
settings-cull-slot-3 = 文件夹 3
settings-cull-slot-3-help = Alt+3 将图像（连同其 RAW 或 JPEG 及附属文件）复制到此文件夹，Alt+Shift+3 将其移动到此处。
settings-cull-slot-3-unset = 未设置
settings-cull-collisions = 名称已被占用时
settings-cull-collisions-help = 文件夹中的文件绝不会被替换。
settings-cull-collisions-suffix = 添加编号
settings-cull-collisions-skip = 跳过该图像
# machine-drafted, needs native review
# 在未打开文件夹或图库图像时选择筛选模式。
cull-nothing = 没有可筛选的内容 — 请先打开一个照片文件夹
cull-slot-unset = 未设置文件夹 { $slot } — 请在“设置 → 筛选模式”中选择
cull-slot-missing = 文件夹 { $slot } 不可用：{ $reason }
cull-copied = 已复制到 { $folder }
cull-moved = 已移动到 { $folder }
cull-slot-renamed = 已在 { $folder } 中另存为 { $name }
cull-slot-skipped = { $folder } 中已有 { $name } — 已跳过
cull-slot-same = 图像已在 { $folder } 中
cull-slot-failed = { $op ->
    [move] 无法移动 { $file }
   *[copy] 无法复制 { $file }
}: { $reason }
cull-copy-kept = 保留了 { $count } 个此后被修改的副本
cull-undo-failed = 无法为 { $file } 撤销：{ $reason }
cull-burst-open = 连拍 { $index } / { $count }
cull-burst-closed = { $count } 张连拍
cull-burst-toggle = 打开或关闭连拍
cull-burst-best = 保留 1 张，拒绝 { $count } 张
cull-burst-none = 不属于连拍
cull-follow-on = 正在跟随新文件
cull-follow-off = 未跟随新文件
cull-new-files = { $count } 个新文件
cull-key-copy = 复制到文件夹 1–3
cull-key-move = 移动到文件夹 1–3
cull-key-burst = 打开或关闭连拍
cull-key-best = 保留这张，拒绝连拍中的其余照片
cull-key-follow = 跟随新文件
cull-key-subframes = 显示或隐藏子帧表
cull-key-sort = 按下一列排序
cull-key-sort-reverse = 反转顺序
cull-key-metric = 图表显示下一个指标
cull-key-sigma-down = 自动排除：降低 k
cull-key-sigma-up = 自动排除：提高 k
cull-key-auto-reject = 排除建议的帧
cull-key-move-rejects = 将排除的帧移到子文件夹
cull-key-same-stretch = 所有帧使用相同拉伸

## Strip tooltips. machine-drafted, needs native review
cull-tip-burst-open = 打开连拍组
cull-tip-burst-close = 关闭连拍组
cull-tip-exit = 退出筛选模式
cull-tip-label = 标签：{ $label }

## Astronomy subframes. machine-drafted, needs native review
cull-astro-panel = 子帧
cull-astro-title = { $count } 个子帧
cull-astro-pending = 正在测量 { $count } 个
cull-astro-col-name = 帧
cull-astro-col-filter = 滤镜
cull-astro-col-exposure = 曝光
cull-astro-col-hfd = HFD
cull-astro-col-fwhm = FWHM
cull-astro-col-ecc = 偏心率
cull-astro-col-stars = 星数
cull-astro-col-bg = 背景
cull-astro-col-snr = SNR
cull-astro-col-noise = 噪声
cull-astro-same-stretch = 相同拉伸
cull-astro-own-stretch = 单独拉伸
cull-astro-suggest-high = { $metric } 高于 { $value }（中位数 + { $k }σ）：{ $count } 帧
cull-astro-suggest-low = { $metric } 低于 { $value }（中位数 − { $k }σ）：{ $count } 帧
cull-astro-suggest-none = 没有超出 { $k }σ 的帧
cull-astro-suggest-few = 自动排除需要 3 个已测量的帧
cull-astro-suggested = 建议排除
cull-astro-k = k { $k }
cull-astro-reject = 排除 { $count } 帧
cull-astro-move = 移动已排除的帧…
cull-astro-chart = { $count } 帧的 { $metric }，中位数 { $median }。
cull-astro-sort-by = 按 { $column } 排序
cull-astro-ascending = 升序
cull-astro-descending = 降序
cull-astro-sorted = 已按 { $column } 排序
cull-astro-reversed = 已反转顺序
cull-astro-metric = 图表：{ $metric }
cull-astro-k-set = 自动排除阈值 { $k }σ
cull-astro-none = 这里没有子帧
cull-astro-same-on = 所有帧使用相同拉伸
cull-astro-same-off = 每帧单独拉伸
cull-astro-line-hfd = HFD { $value } px
cull-astro-line-fwhm = FWHM { $value } px
cull-astro-line-fwhm-arcsec = FWHM { $value } px（{ $arcsec }″）
cull-astro-line-ecc = 偏心率 { $value }
cull-astro-line-stars = { $count } 颗星
cull-astro-measuring = 正在测量星点…
cull-astro-reject-confirm = 排除建议的 { $count } 帧？
cull-astro-reject-detail = { $rule }。撤销可恢复标记。
cull-astro-reject-ok = 排除
cull-astro-nothing-to-reject = 没有可排除的帧
cull-astro-rejected = 已排除 { $count } 帧
cull-move-rejects-confirm = 将已排除的 { $count } 帧移到文件夹“{ $folder }”？
cull-move-rejects-detail = 文件只会移动，不会删除。撤销可将其移回。
cull-move-rejects-ok = 移动
cull-move-rejects-none = 没有可移动的已排除帧
cull-moved-rejects = 已将 { $count } 帧移到“{ $folder }”
