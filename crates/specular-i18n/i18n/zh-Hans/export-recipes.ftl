# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = 按方案导出…
palette-export-recipes-help = 按一个或多个已保存的方案导出所选内容。
palette-develop-export-recipes = 按方案导出…
palette-develop-export-recipes-help = 按已保存的方案导出编辑过的照片（一张或多张）。

export-recipes-title = 按方案导出
export-recipes-new = 新建
export-recipes-edit = 编辑…
export-recipes-duplicate = 复制
export-recipes-delete = 删除…
export-recipes-start = 导出
export-recipes-save = 保存
export-recipes-open-folder = 打开文件夹
export-recipes-choose-image = 选择图像…
export-recipes-no-images = 没有可导出的图像。
export-recipes-none-checked = 请至少勾选一个方案。
export-recipes-checked = { $name }：开
export-recipes-unchecked = { $name }：关

## The starters.
export-recipes-starter-web = 网页，2048 px
export-recipes-starter-print = 打印，完整尺寸
export-recipes-starter-email = 电子邮件，小尺寸
export-recipes-unnamed = 未命名方案
export-recipes-new-name = 新方案
# { $name } is the recipe copied.
export-recipes-copy-name = { $name }（副本）

## The list's one-line summary.
export-recipes-summary-mark = 水印
export-recipes-size-full = 完整尺寸
export-recipes-size-long = 长边 { $px } px
export-recipes-size-short = 短边 { $px } px
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = 原文件旁边
export-recipes-folder-sub = 文件夹“{ $name }”
export-recipes-folder-ask = 每次询问

## The editor.
export-recipes-new-title = 新方案
export-recipes-edit-title = 编辑方案
export-recipes-name = 方案名称
export-recipes-text = 自定义文本
export-recipes-pattern = 文件名
export-recipes-mark-text-field = 水印文本
export-recipes-subfolder = 文件夹名称
export-recipes-format = 格式
export-recipes-quality = 质量
export-recipes-lossless = 无损
export-recipes-bits = 位深度
export-recipes-bits-8 = 8 位
export-recipes-bits-16 = 16 位
export-recipes-space = 色彩空间
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = 描述文件
export-recipes-profile-embed = 嵌入
export-recipes-profile-none = 不包含
export-recipes-folder = 文件夹
export-recipes-folder-subfolder = 子文件夹
export-recipes-collision = 名称已被占用
export-recipes-collision-unique = 添加编号
export-recipes-collision-overwrite = 替换（会询问）
export-recipes-collision-skip = 跳过
export-recipes-after = 完成后
export-recipes-after-nothing = 不执行操作
export-recipes-after-open = 打开文件夹
export-recipes-resize = 尺寸
export-recipes-resize-long = 长边
export-recipes-resize-short = 短边
export-recipes-resize-mp = 百万像素
export-recipes-resize-exact = 精确尺寸
export-recipes-resize-percent = 百分比
export-recipes-edge = 边长
export-recipes-width = 宽度
export-recipes-height = 高度
export-recipes-megapixels = 百万像素
export-recipes-percent = 百分比
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = 允许放大
export-recipes-sharpen = 锐化用于
export-recipes-sharpen-off = 关
export-recipes-sharpen-screen = 屏幕
export-recipes-sharpen-matte = 亚光纸
export-recipes-sharpen-glossy = 光面纸
export-recipes-amount = 程度
export-recipes-amount-low = 低
export-recipes-amount-standard = 标准
export-recipes-amount-high = 高
export-recipes-metadata = 元数据
export-recipes-meta-all = 全部
export-recipes-meta-copyright = 仅版权
export-recipes-meta-contact = 版权和联系信息
export-recipes-meta-none = 无
export-recipes-location = 位置
export-recipes-people = 照片中的人物
export-recipes-keep = 保留
export-recipes-remove = 移除
export-recipes-mark = 水印
export-recipes-mark-none = 无
export-recipes-mark-text = 文本
export-recipes-mark-image = 图像（请选择）
export-recipes-position = 位置
export-recipes-pos-top-left = 左上
export-recipes-pos-top = 上
export-recipes-pos-top-right = 右上
export-recipes-pos-left = 左
export-recipes-pos-centre = 居中
export-recipes-pos-right = 右
export-recipes-pos-bottom-left = 左下
export-recipes-pos-bottom = 下
export-recipes-pos-bottom-right = 右下
export-recipes-opacity = 不透明度
export-recipes-scale = 宽度
export-recipes-yes = 是
export-recipes-no = 否
export-recipes-error-name = 请为方案命名。
export-recipes-error-mark-text = 请输入水印文本。
export-recipes-error-mark-image = 请选择水印图像。
export-recipes-error-subfolder = 请输入文件夹名称。
export-recipes-saved = 已保存方案“{ $name }”。

## Delete.
export-recipes-delete-title = 删除方案“{ $name }”？
export-recipes-delete-detail = 它导出的文件会保留在磁盘上。
export-recipes-delete-confirm = 删除
export-recipes-delete-keep = 保留
export-recipes-deleted = 已删除方案“{ $name }”。

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = “{ $name }”的文件夹
export-recipes-asking = 为每个需要询问的方案选择文件夹。
export-recipes-no-folder = 未选择文件夹，未导出任何内容。
export-recipes-planning = 正在命名文件…
export-recipes-plan-pattern = 方案“{ $name }”：{ $reason }
export-recipes-replace-title = { $count ->
   *[other] 替换 { $count } 个文件？
}
export-recipes-replace-detail = 原文件绝不会被替换。
export-recipes-replace-confirm = 替换
export-recipes-replace-kept = 未导出任何内容。
export-recipes-running = 正在导出…
export-recipes-progress = 第 { $done } 个文件，共 { $total } 个
export-recipes-done = { $count ->
   *[other] 已导出 { $count } 个文件。
}
export-recipes-stopped = { $count ->
   *[other] 已停止。已导出 { $count } 个文件。
}
export-recipes-done-skipped = { $count ->
   *[other] 跳过 { $count } 个：名称已被占用。
}
export-recipes-done-failed = { $count ->
   *[other] { $count } 个失败：
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file }（{ $recipe }）：{ $reason }
