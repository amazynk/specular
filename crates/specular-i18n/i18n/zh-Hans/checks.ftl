# machine-drafted, needs native review

## Model checks on IFC building models (mesh mode, Alt+Q): IDS, clashes, changes between
## versions, quantities. IFC class names (IfcWall), property set names and values are data
## and stay as the file has them. Keys named here are the defaults.

# The panel's tabs.
checks-tab = { $tab ->
    [ids] IDS
    [clashes] 碰撞
    [changes] 变更
   *[quantities] 工程量
}
# $tab: the tab's name.
checks-title = 模型检查 · { $tab }

# Short texts, by $what.
checks-note = { $what ->
    [not-ifc] 模型检查需要 IFC 模型。
    [reading] 正在读取模型信息…
    [ids-empty] 没有 IDS 文件。按 O 打开一个。
    [ids-checking] 正在检查…
    [ids-no-failures] 没有不通过项。
    [ids-hints] Enter 不通过项、前往 · Backspace 返回 · O 打开 · P 属性 · Ctrl+S BCF · Ctrl+E CSV · Tab 下一个
    [unnamed] 未命名规范
    [clash-running] 正在查找碰撞…
    [clash-empty] 按 R 查找碰撞。
    [clash-none] 没有碰撞。
    [clashes-hints] R 运行 · A B 集合 · M 硬碰撞或间隙 · + − 距离 · Enter 前往 · S 状态 · Ctrl+S BCF · Ctrl+E CSV · Tab 下一个
    [changes-reading] 正在读取另一版本…
    [changes-empty] 按 O 选择此模型的另一版本。
    [changes-none] 没有变更。
    [changes-hints] O 另一版本 · F 筛选 · Enter 前往 · P 属性 · Ctrl+S BCF · Ctrl+E CSV · Tab 下一个
    [quantities-hints] G 分组 · Enter 前往 · P 属性 · Ctrl+E CSV · Ctrl+S XLSX · Tab 下一个
   *[not-drawn] 此构件没有可显示的几何。
}

# What a requirement is about. $name: an attribute name, an IFC class or a pattern.
checks-facet = { $facet ->
    [entity] 类
    [attribute] 属性 { $name }
    [classification] 分类
    [material] 材料
   *[partof] { $name } 的一部分
}

# Why an element fails a requirement. $what: what the requirement is about; $actual: what
# the element has.
checks-reason = { $why ->
    [class] 类为 { $actual }
    [predefined] 预定义类型为 { $actual }
    [missing] 缺少 { $what }
    [empty] { $what } 为空
    [value] { $what } 为 { $actual }：不允许
    [datatype] { $what } 的数据类型为 { $actual }
    [system] { $what }：体系为 { $actual }
    [whole] { $what }：属于 { $actual }
    [prohibited] 不得存在 { $what }
   *[unsupported] 无法检查 { $what }
}

# Shown for a missing value.
checks-none = —

# A fact about the chosen element. $value: its value.
checks-prop = { $what ->
    [storey] 楼层：{ $value }
    [type] 类型：{ $value }
    [material] 材料：{ $value }
    [container] 容器：{ $value }
   *[classification] 分类：{ $value }
}

# $file: the IDS file; $ok: specifications that hold; $count: all of them.
checks-ids-summary = { $file }：{ $count } 项中 { $ok } 项满足

# A specification's result. $applicable: objects it applies to; $failed: those failing.
checks-spec = { $state ->
    [pass] ✓ { $applicable } 个通过
    [fail] ✗ { $applicable } 个中 { $failed } 个不通过
    [none] ✗ 不适用于任何对象
    [none-ok] ✓ 不适用于任何对象
    [prohibited] ✗ { $applicable } 个不应存在
   *[prohibited-ok] ✓ 没有不应存在的对象
}

# A side of a clash test. $name: an IFC class or a storey's name.
checks-set = { $kind ->
    [all] 所有构件
    [class] { $name }
   *[storey] 楼层 { $name }
}
# $a, $b: the two sides; $distance: the tolerance or clearance with its unit.
checks-clash-setup = A：{ $a } · B：{ $b } · { $mode ->
    [hard] 硬碰撞，深于 { $distance }
   *[clearance] 近于 { $distance }
}
# $count: clashes found; $new: new ones; $resolved: earlier ones no longer found; $seconds: time taken.
checks-clash-count = { $count ->
   *[other] { $count } 处碰撞
}，新 { $new }，已解决 { $resolved } · { $seconds } 秒
# One clash. $depth: the depth or distance with its unit; $status: its status.
checks-clash = { $kind ->
    [hard] 硬碰撞 { $depth }
   *[clearance] 间隙 { $depth }
} · { $status }
checks-clash-kind = { $kind ->
    [hard] 硬碰撞
   *[clearance] 间隙
}
checks-status = { $status ->
    [new] 新
    [active] 活动
   *[resolved] 已解决
}
# A BCF topic's title for a clash. $a, $b: the two elements.
checks-clash-topic = 碰撞：{ $a } × { $b }
checks-clash-done = { $count ->
    [0] 没有碰撞
   *[other] { $count } 处碰撞
}

# $file: the other version; counts of added, removed and changed elements; $filter: what is listed.
checks-changes-summary = { $file }：新增 { $added } · 删除 { $removed } · 变更 { $changed }{ $filter ->
    [added] {" "}· 显示新增
    [removed] {" "}· 显示删除
    [changed] {" "}· 显示变更
   *[all] {""}
}
checks-change = { $kind ->
    [added] 新增
    [removed] 删除
   *[geometry] 几何
}
checks-change-fields = { $count ->
   *[other] { $count } 个字段
}
# A field that changed. $name: the field; $old, $new: its values.
checks-field = { $name }：{ $old } → { $new }
# $distance: how far it moved, with its unit.
checks-moved = 移动了 { $distance }

# $by: the grouping; $count: elements; $area, $volume: totals with units.
checks-qto-summary = { $count } 个构件 · { $area } · { $volume } · { $by ->
    [element] 按构件
    [class] 按类
    [storey] 按楼层
   *[type] 按类型
}
# Where quantities came from.
checks-source = { $source ->
    [file] 来自文件
    [computed] 计算所得
   *[mixed] 部分计算所得
}
# A sheet of the quantities workbook.
checks-group = { $by ->
    [element] 构件
    [class] 按类
    [storey] 按楼层
   *[type] 按类型
}

# Column headers of exported tables.
checks-col = { $col ->
    [spec] 规范
    [guid] GlobalId
    [class] 类
    [name] 名称
    [reason] 原因
    [a] A GlobalId
    [a-name] A
    [b] B GlobalId
    [b-name] B
    [kind] 类别
    [depth] 深度或距离 (m)
    [status] 状态
    [x] X (m)
    [y] Y (m)
    [z] Z (m)
    [change] 变更
    [field] 字段
    [old] 旧值
    [new] 新值
    [type] 类型
    [container] 容器
    [material] 材料
    [classification] 分类
    [storey] 楼层
    [count] 数量
    [length] 长度 (m)
    [area] 面积 (m²)
    [volume] 体积 (m³)
   *[source] 来源
}

# $reason: why, in plain words.
checks-error = 无法执行：{ $reason }
# A results summary for screen readers. $failed: specifications that do not hold.
checks-ids-done = { $failed ->
    [0] 检查完成：所有规范均满足
   *[other] 检查完成：{ $failed } 项规范不满足
}
# $file: the file written.
checks-saved = 已保存 { $file }

# Several models in one scene. An element or clash set and the model file it is from.
# machine-drafted, needs native review
checks-in-model = { $element }（{ $model }）
# A clash set taken from every model of the scene.
# machine-drafted, needs native review
checks-every-model = 所有模型
# The model IDS, changes and quantities are about. $model: its file name.
# machine-drafted, needs native review
checks-model = 模型：{ $model }
# A tab's keys in a scene of several models, by $tab (ids, clashes, changes, quantities).
# machine-drafted, needs native review
checks-hints-models = { $tab ->
    [ids] M 模型 · Enter 不通过项、前往 · Backspace 返回 · O 打开 · P 属性 · Ctrl+S BCF · Ctrl+E CSV · Tab 下一个
    [clashes] R 运行 · A B 集合 · G H 各自模型 · M 硬碰撞或间隙 · + − 距离 · Enter 前往 · S 状态 · Ctrl+S BCF · Ctrl+E CSV · Tab 下一个
    [changes] M 模型 · O 另一版本 · F 筛选 · Enter 前往 · P 属性 · Ctrl+S BCF · Ctrl+E CSV · Tab 下一个
   *[quantities] M 模型 · G 分组 · Enter 前往 · P 属性 · Ctrl+E CSV · Ctrl+S XLSX · Tab 下一个
}
