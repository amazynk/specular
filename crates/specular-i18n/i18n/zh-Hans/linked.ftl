# Linked views: several DICOM series side by side, scrolling together by position, with
# reference lines, a 3D cursor, window and zoom sync, and manual alignment of priors.

# machine-drafted, needs native review
palette-compare-series = 比较序列（联动视图）
# machine-drafted, needs native review
palette-compare-layout = 下一个视图布局
# machine-drafted, needs native review
palette-next-viewport = 下一个视图
# machine-drafted, needs native review
palette-prev-viewport = 上一个视图
# machine-drafted, needs native review
palette-reference-lines = 显示或隐藏参考线
# machine-drafted, needs native review
palette-sync-window = 下一个窗同步方式
# machine-drafted, needs native review
palette-sync-zoom = 按大小一起缩放和平移
# machine-drafted, needs native review
palette-link-series = 下一个序列联动方式
# machine-drafted, needs native review
palette-align-series = 将其他序列对齐到此处
# machine-drafted, needs native review
palette-compare-series-help = 并排显示多个序列，一起滚动。
# machine-drafted, needs native review
palette-compare-layout-help = 显示更多或更少的视图。
# machine-drafted, needs native review
palette-next-viewport-help = 将下一个视图设为活动视图。
# machine-drafted, needs native review
palette-prev-viewport-help = 将上一个视图设为活动视图。
# machine-drafted, needs native review
palette-reference-lines-help = 绘制活动层面与其他视图相交的位置。
# machine-drafted, needs native review
palette-sync-window-help = 选择哪些视图共享窗宽/窗位的更改：关、同一模态或全部。
# machine-drafted, needs native review
palette-sync-zoom-help = 以相同大小显示所有视图，并一起平移。
# machine-drafted, needs native review
palette-link-series-help = 选择此序列如何跟随其他序列。
# machine-drafted, needs native review
palette-align-series-help = 在显示的层面上，将先前的序列与此序列对齐。
# machine-drafted, needs native review
disabled-not-comparing = 请先比较序列

# The bar above the viewports. Toggles show their state by colour; the others name it.
# machine-drafted, needs native review
linked-layout = 布局：{ $cols } × { $rows }
# machine-drafted, needs native review
linked-lines = 参考线
# machine-drafted, needs native review
linked-zoom = 缩放同步
# machine-drafted, needs native review
linked-window-sync = 窗同步： { $mode ->
    [off] 关
    [modality] 同一模态
   *[all] 全部
}
# machine-drafted, needs native review
linked-link = 联动： { $mode ->
    [frame] 参考坐标系
    [position] 患者体位
   *[off] 关
}
# machine-drafted, needs native review
linked-align = 对齐到此处

# One viewport. $active is "yes" for the active one.
# machine-drafted, needs native review
linked-viewport = { $active ->
    [yes] 视图 { $n }/{ $count }，活动
   *[no] 视图 { $n }/{ $count }
}
# machine-drafted, needs native review
linked-empty = 将序列拖到此处
# machine-drafted, needs native review
linked-series = { $number } · { $modality } · { $description }
# machine-drafted, needs native review
linked-slice = 层面 { $n }/{ $count } · { $position } mm
# machine-drafted, needs native review
linked-in-gap = 位于间隙中：最近的层面距此 { $distance } mm
# machine-drafted, needs native review
linked-outside = 超出此序列范围（{ $distance } mm）
# machine-drafted, needs native review
linked-not-linked = 未联动
# machine-drafted, needs native review
linked-aligned = 已手动对齐（{ $offset } mm）
# machine-drafted, needs native review
linked-cursor = 3D 光标位于 { $x }, { $y }, { $z } mm
# machine-drafted, needs native review
linked-chip = { $label }，{ $count } 幅图像

# Toasts and announcements.
# machine-drafted, needs native review
linked-opened = { $count ->
    [1] 此处只有一个序列：请从检查浏览器再拖入一个
   *[other] 正在比较 { $count } 个序列
}
# machine-drafted, needs native review
linked-no-geometry = 此序列没有层面位置，无法联动
# machine-drafted, needs native review
linked-active = 视图 { $n }：{ $series }
# machine-drafted, needs native review
linked-aligned-count = { $count ->
    [0] 无需对齐：其他序列共享此参考坐标系
    [1] 已将 1 个序列与活动序列对齐（仅限本次会话）
   *[other] 已将 { $count } 个序列与活动序列对齐（仅限本次会话）
}
