# machine-drafted, needs native review
## 审阅三维模型：漫游和飞行、剖切面和剖切框、已保存的视图、BCF 问题
## （三维模式）。文中所列按键均为默认按键。

cheatsheet-review = 审阅

palette-mesh-walk = 漫游
palette-mesh-fly = 飞行
palette-mesh-views = 已保存的视图
palette-mesh-save-view = 保存视图
palette-mesh-section-plane = 添加剖切面
palette-mesh-section-box = 剖切框
palette-mesh-section-flip = 翻转剖切面
palette-mesh-section-clear = 移除所有剖切
palette-mesh-measure-cut = 在剖切面上测量
palette-mesh-bcf-topics = BCF 问题
# machine-drafted, needs native review
palette-mesh-model-checks = 模型检查
# machine-drafted, needs native review
palette-mesh-models = 场景中的模型
# machine-drafted, needs native review
palette-mesh-add-model = 添加模型…
# machine-drafted, needs native review
palette-mesh-add-model-folder = 从文件夹添加模型…

# 漫游和飞行：提示和读数行，按 $what。$eye：眼睛离地面的高度，带单位（仅 walk-line）。
mesh-nav = { $what ->
    [walk-toast] 漫游中：W A S D 移动，拖动环顾，Esc 停止
    [fly-toast] 飞行中：W A S D 移动，Q E 下降和上升，Esc 停止
    [orbit] 环绕
    [walk-line] 漫游中 · 眼高离地 { $eye }
    [walk-keys] W A S D 移动 · 拖动环顾 · Shift 加速 · Esc 停止
    [preparing] 墙体尚未就绪：会穿墙而过
    [fly-keys] W A S D 移动 · Q E 下降、上升 · 拖动环顾 · Shift 加速 · Esc 停止
   *[fly-line] 飞行中
}

# 剖切、测量和已保存视图的短文本，按 $what。
bim-note = { $what ->
    [plane-added] 已添加剖切面：拖动其手柄可移动
    [box-on] 剖切框已开启：拖动某个面的手柄可移动该面
    [box-off] 剖切框已关闭
    [cleared] 已移除剖切
    [measure-on] 在剖切面上点击两个点
    [measure-needs-cut] 请先添加剖切面或剖切框
    [measure-off-cut] 请点击剖切面
    [measure-hint] 测量中：在剖切面上点击两个点
    [ask-view-name] 视图名称
    [ask-rename] 新名称
    [views-empty] 没有已保存的视图。按 N 保存当前视图。
    [views-hints] Enter 前往 · N 保存 · F2 重命名 · Delete 移除 · P 另存为 PNG
    [delete-detail] 只移除已保存的视图，模型不会改变。
    [delete-ok] 移除
    [not-over-model] 请选择其他文件：此文件会替换模型。
   *[view-name] 视图
}

# BCF（BIM Collaboration Format）短文本，按 $what。
bcf-note = { $what ->
    [title-none] BCF 问题
    [new-file] 新建
    [empty] 没有问题。按 O 打开 BCF 文件，或按 N 添加问题。
    [hints] Enter 前往 · ← → 视点 · N 问题 · C 评论 · S 状态 · E 标题 · O 打开 · Ctrl+S 导出
    [ask-title] 问题标题
    [ask-comment] 评论
    [untitled] 无标题
    [topic-added] 已添加问题及当前视图
   *[comment-added] 已添加评论及当前视图
}

# $planes：剖切面数量。
bim-sections-line = { $planes ->
    [0] 没有剖切面
   *[other] { $planes } 个剖切面
}
# $size：剖切框的大小，带单位。
bim-box-line = 剖切框 { $size }
# $count：剖切面的最大数量。
bim-planes-full = 最多 { $count } 个剖切面
# 拖动手柄，供屏幕阅读器使用。$n：其编号。
bim-handle = { $kind ->
    [box] 剖切框面 { $n }
   *[plane] 剖切面 { $n }
}
# $length：带单位的长度。
bim-measure-line = 剖切面上的长度：{ $length }
bim-views-title = { $count ->
   *[other] { $count } 个已保存的视图
}
# $name：视图名称。
bim-view-saved = 已保存“{ $name }”
bim-view-delete-title = 移除视图“{ $name }”？
# $file：写入的文件。
bim-saved-file = 已保存 { $file }
# $reason：失败原因。
bim-failed = { $what ->
    [open] 无法打开 BCF 文件：{ $reason }
    [views] 视图仅在本次会话中保留：{ $reason }
   *[save] 无法保存：{ $reason }
}
# $count：问题数；$file：归档文件名。
bcf-title = { $count ->
   *[other] { $count } 个问题
} · { $file }
bcf-comments = { $count ->
   *[other] { $count } 条评论
}
# $at：显示的视点；$count：问题的视点数。
bcf-viewpoint-of = 视点 { $at } / { $count }
bcf-opened = { $count ->
   *[other] 已打开 { $count } 个问题
}

## Several IFC models in one scene (the Models list, Alt+Shift+M).

# The list's title. $count: models in the scene.
# machine-drafted, needs native review
fed-title = { $count ->
   *[other] 模型：{ $count }
}
# machine-drafted, needs native review
fed-note = { $what ->
    [not-ifc] 此文件不是 IFC 模型。
    [not-ifc-open] 请先打开 IFC 模型。只有 IFC 模型可以组合。
    [this-model] 此模型
    [hints] A 添加模型 · Shift+A 添加文件夹 · Space 显示或隐藏 · C 按模型着色 · Enter 前往 · Delete 移除
    [origin-ok] 放在原点
    [first-stays] 已打开的模型保持不变。要更换它，请打开其他文件。
    [remove-detail] 它将离开场景，磁盘上的文件会保留。
   *[remove-ok] 移除
}
# A model's row. $basis: how it was placed; $elements: its elements; $shown: shown or hidden.
# machine-drafted, needs native review
fed-row = { $basis } · { $elements ->
   *[other] { $elements } 个构件
} · { $shown ->
    [hidden] 已隐藏
   *[shown] 已显示
}
# How a model was placed against the first. $crs: the coordinate system's name.
# machine-drafted, needs native review
fed-basis = { $basis ->
    [reference] 已打开的模型
    [map-crs] 按地图坐标（{ $crs }）
    [map] 按地图坐标
    [site] 按场地经纬度（近似）
    [project] 按共享项目坐标
   *[origin] 位于原点（未对齐）
}
# Why a model cannot be aligned. $first, $this: coordinate system names.
# machine-drafted, needs native review
fed-unaligned = { $why ->
    [crs] 模型使用不同的坐标系：{ $first } 和 { $this }。
    [only-first] 已打开的模型有地理参考，此模型没有。
    [only-this] 此模型有地理参考，已打开的模型没有。
   *[broken] 此模型的地图转换无法使用（其比例为零）。
}
# The chosen model's placement. $distance: how far it moved; $angle: degrees turned.
# machine-drafted, needs native review
fed-placed = 移动 { $distance }，旋转 { $angle }°
# machine-drafted, needs native review
fed-adding = { $count ->
   *[other] 正在读取 { $count } 个模型…
}
# machine-drafted, needs native review
fed-ask-origin = 无法对齐 { $file }。要将其放在原点吗？
# machine-drafted, needs native review
fed-no-files = { $folder } 中没有 IFC 文件。
# machine-drafted, needs native review
fed-not-added = 未添加 { $file }：{ $reason }
# machine-drafted, needs native review
fed-added = 已添加 { $file }。场景中共有 { $count } 个模型。
# machine-drafted, needs native review
fed-remove-title = 从场景中移除 { $file }？
# machine-drafted, needs native review
fed-removed = 已从场景中移除 { $file }。
# machine-drafted, needs native review
fed-shown = { $file } { $shown ->
    [hidden] 已隐藏
   *[shown] 已显示
}
# machine-drafted, needs native review
fed-colours = { $on ->
    [on] 按模型着色：开
   *[off] 按模型着色：关
}

## PMI (product manufacturing information) of a CAD model: dimensions, geometric tolerances,
## datums and the file's saved views (mesh mode).

# machine-drafted, needs native review
palette-mesh-pmi = PMI 和已保存的视图
# $count: annotations in the model.
# machine-drafted, needs native review
pmi-title = { $count ->
   *[other] { $count } 个标注
}
# machine-drafted, needs native review
pmi-view = 已保存的视图
# machine-drafted, needs native review
pmi-view-all = 显示所有标注
# $count: annotations the saved view shows.
# machine-drafted, needs native review
pmi-view-shows = { $count ->
   *[other] 显示 { $count } 个标注
}
# $name: the saved view's name.
# machine-drafted, needs native review
pmi-view-shown = 正在显示视图“{ $name }”。按 A 显示全部。
# machine-drafted, needs native review
pmi-has-faces = 已高亮面
# Short texts, by $what.
# machine-drafted, needs native review
pmi-note = { $what ->
    [none] 此模型中没有 PMI。
    [hints] ↑ ↓ 选择 · Enter 前往 · A 全部 · D 按原样绘制 · Shift+A 隐藏
    [hidden] 标注已隐藏
    [shown] 标注已显示
    [as-drawn] 按文件的绘制方式显示标注
    [as-labels] 以标签显示标注
   *[all-shown] 所有标注已显示
}
# What an annotation is, by $what.
# machine-drafted, needs native review
pmi-kind = { $what ->
    [dimension] 尺寸
    [tolerance] 几何公差
    [datum] 基准
    [target] 基准目标
   *[graphic] 图形标注
}
# machine-drafted, needs native review
pmi-dimension = { $what ->
    [linear] 距离
    [angular] 角度
    [diameter] 直径
    [radius] 半径
    [length] 长度
    [thickness] 厚度
   *[other] 尺寸
}
# Geometric characteristics (ISO 1101), by $what.
# machine-drafted, needs native review
pmi-characteristic = { $what ->
    [straightness] 直线度
    [flatness] 平面度
    [circularity] 圆度
    [cylindricity] 圆柱度
    [line-profile] 线轮廓度
    [surface-profile] 面轮廓度
    [angularity] 倾斜度
    [perpendicularity] 垂直度
    [parallelism] 平行度
    [position] 位置度
    [concentricity] 同心度
    [coaxiality] 同轴度
    [symmetry] 对称度
    [circular-runout] 圆跳动
   *[total-runout] 全跳动
}
