# machine-drafted, needs native review
# Photographer polish pass: Develop regions, Crop card rows, import choices, keyword rename and move, auto-apply rules.

# Develop: F6 between the picture, the panel and the filmstrip.
palette-develop-next-region = 下一个区域
palette-develop-next-region-help = 将键盘焦点前移：画面、面板、胶片带。
palette-develop-prev-region = 上一个区域
palette-develop-prev-region-help = 将键盘焦点后移：胶片带、面板、画面。
develop-region-picture = 画面
develop-region-panel = 面板

# Develop: the Crop card's rows (the crop tool's keys as rows).
develop-crop-aspect-row = 长宽比
develop-crop-aspect-option-free = 自由
develop-crop-aspect-option-original = 原始
develop-crop-aspect-option-custom = 自定义
develop-crop-type-ratio = 输入比例…
develop-crop-swap = 互换纵向和横向
develop-crop-turn-left = 向左旋转
develop-crop-turn-right = 向右旋转
develop-crop-flip-horizontal = 左右翻转
develop-crop-flip-vertical = 上下翻转
develop-crop-ratio-title = 裁剪长宽比
develop-crop-ratio-field = 宽:高
develop-crop-ratio-ok = 设置
develop-crop-ratio-bad = 请输入比例，如 5:4，或一个数字，如 1.85。

# Import: what each copied photo gets.
import-template = 元数据模板
import-style = 修改照片风格
import-choice-none = 无
# A choice row stepped, read out: "Metadata template: Copyright".
import-choice-said = { $label }：{ $value }
import-keywords = 关键词
import-keywords-hint = 例如 家人; 地点 > 巴黎
import-style-missing = 风格 { $name } 已不存在。照片已在不使用它的情况下导入。

# Keywords: rename, move and merge.
keywords-rename = 重命名…
keywords-move = 移动到…
keywords-rename-of = { $keyword } 的新名称
keywords-move-of = 将 { $keyword } 放到以下关键词之下（留空：顶层）
keywords-move-into-itself = { $keyword } 不能放到自身之下。
keywords-renaming = 正在将 { $from } 重命名为 { $to }…
keywords-merging = 正在将 { $from } 合并到 { $to }…
keywords-renamed = { $count ->
   *[other] { $count } 张照片上的 { $from } 已改为 { $to }
}
keywords-rename-row = 关键词 { $from } 改为 { $to }
keywords-rename-finding = 正在查找照片…
keywords-rename-row-detail = { $done } / { $total } 张照片

# Develop: the before/after divider dragged ("Divider at 30 %").
develop-split-moved = 分隔线位于 { PERCENT($percent) }

# Develop: auto-apply rules, changed row by row.
develop-rules-placeholder-edit = 选择一条规则以修改或删除
develop-rule-edit-placeholder = { $rule }
develop-rule-maker = 制造商：{ $value }
develop-rule-model = 型号：{ $value }
develop-rule-lens = 镜头：{ $value }
develop-rule-iso = ISO：{ $value }
develop-rule-type = 文件类型：{ $value }
develop-rule-style = 风格：{ $value }
develop-rule-any = 任意
develop-rule-type-any = 任意文件
develop-rule-delete = 删除规则…
develop-rule-field-title = 修改“{ $rule }”
develop-rule-field-maker = 制造商（名称的一部分即可；留空：任意）
develop-rule-field-model = 型号（名称的一部分即可；留空：任意）
develop-rule-field-lens = 镜头（名称的一部分即可；留空：任意）
develop-rule-field-iso = ISO，如 100-800、1600- 或 -400（留空：任意）
develop-rule-field-ok = 设置
develop-rule-iso-bad = 请输入 ISO 范围，如 100-800。
develop-rule-style-placeholder = 选择规则的风格
