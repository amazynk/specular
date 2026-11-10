# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = 相册
albums-new = 新建相册…
albums-new-smart = 新建智能相册…
albums-save-search = 将搜索存为智能相册…
albums-smart = 智能相册
# The album the photos go to when added without choosing.
albums-target = 添加到这里
albums-group-count = { $count ->
   *[other] { $count } 个相册
}
albums-need-library = 相册位于图库中。请先将此文件夹添加到图库。

## Screen readers: the buttons of album and group rows
albums-a11y-rename = 重命名 { $album }
albums-a11y-edit = 编辑 { $album }
albums-a11y-delete = 删除相册 { $album }
albums-a11y-rename-group = 重命名分组 { $group }
albums-a11y-delete-group = 删除分组 { $group }

## Sheets
albums-name = 名称
albums-unnamed = 尚未命名
albums-new-title = 新建相册
albums-new-smart-title = 新建智能相册
albums-edit-smart-title = 编辑智能相册
albums-rename-title = 重命名相册
albums-rename-group-title = 重命名分组
albums-create = 创建
albums-rename = 重命名
albums-save = 保存
albums-groups-note = 用 / 将其放入分组：客户 X / 精选。
albums-name-empty = 请输入名称。
albums-name-taken = { $album } 已存在。
albums-name-clash = { $album } 是相册，因此不能同时是分组。

## Smart album rules
albums-rules = 规则
albums-rules-note = 规则从上到下依次应用。空规则会被忽略。
albums-rules-none = 请至少添加一条规则。
albums-rule-label = 规则 { $number }
albums-rule-empty = 空规则
albums-rule-add = 添加规则
albums-rule-combine = 与上方规则组合
albums-rule-first = 符合以下条件的照片
albums-rule-and = 并且
albums-rule-or = 或者
albums-rule-except = 排除

## Adding and removing photos
albums-add-menu = 添加到相册…
albums-remove-menu = 从相册中移除
albums-add-title = { $count ->
   *[other] 将 { $count } 张照片添加到相册
}
albums-add = 添加
albums-add-new = 新建相册
albums-add-list = 相册
albums-add-none = 还没有相册。输入名称即可创建一个。
albums-add-note = 或输入新相册的名称。
albums-added = { $count ->
   *[other] 已将 { $count } 张照片添加到 { $album }。
}
albums-added-missing = { $count ->
   *[other] 有 { $count } 张不在图库中，因此未添加。
}
albums-created = 已创建 { $album }。
albums-removed = { $count ->
   *[other] 已从 { $album } 中移除 { $count } 张照片。
}
albums-remove = 移除
albums-remove-confirm = { $count ->
   *[other] 要从 { $album } 中移除 { $count } 张照片吗？
}
albums-remove-confirm-detail = 文件仍保留在磁盘和图库中。
albums-delete = 删除
albums-delete-confirm = 要删除相册 { $album } 吗？
albums-delete-group-confirm = { $count ->
   *[other] 要删除分组 { $group } 及其中的 { $count } 个相册吗？
}
albums-delete-confirm-detail = 照片仍保留在磁盘和图库中。
albums-deleted = { $count ->
    [1] 已删除相册。
   *[other] 已删除 { $count } 个相册。
}
albums-failed = 无法更改相册：{ $reason }

## Date timeline
albums-timeline = 时间线
albums-timeline-description = 选择一个时间段，只显示其中的照片。Shift+点击可连续选择多个。
albums-timeline-picked = 正在显示 { $period }
albums-timeline-cleared = 正在显示所有日期
