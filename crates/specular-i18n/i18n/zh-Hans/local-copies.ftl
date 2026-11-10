# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = 本地副本上限
settings-library-local-copies-limit-help = 本地副本在 Specular 数据文件夹中最多可占用的空间。

local-keep-menu = 保留本地副本…
local-remove-menu = 移除本地副本…
local-keep-location-menu = 保留此位置的本地副本…
local-remove-location-menu = 移除此位置的本地副本…
local-keep-album-menu = 保留此相册的本地副本…
local-remove-album-menu = 移除此相册的本地副本…
local-remove-held-elsewhere = 这些副本属于已保留的位置或相册。请在那里移除。

local-keep-ask = 要保留 { $item } 的本地副本吗？
local-keep-detail = { $count ->
   *[other] { $count } 张照片。
}
    原图：按原样复制文件。
    代理文件：RAW 以 2560 像素保存，足够修改照片。其他文件完整复制。
    副本存放在 Specular 的数据文件夹中。原图不会更改。
local-keep-originals = 保留原图
local-keep-proxies = 保留代理文件
local-keeping = 正在复制 { $item }…
local-kept = { $item }：{ $count ->
   *[other] { $count } 个本地副本
}，{ $size }。
local-kept-full = { $count ->
   *[other] { $count } 张照片超出了大小上限。
}
local-kept-unreachable = { $count ->
   *[other] { $count } 张照片无法读取。
}

local-remove-ask = 要移除 { $item } 的本地副本吗？
local-remove-detail = Specular 数据文件夹中的副本将被删除。原图不受影响。等待中的更改会保留。
local-remove-ok = 移除本地副本
local-removed = 已移除 { $item } 的本地副本，释放了 { $size }。

local-banner-offline = { $location } 已离线。你正在使用本地副本。
local-banner-offline-many = { $count } 个位置已离线。你正在使用本地副本。
local-banner-waiting = { $count ->
   *[other] { $count } 项更改正在等待原图。
}
local-banner-conflicts = { $count ->
   *[other] 离线期间有 { $count } 个文件发生了更改。
}
local-review = 查看…
local-sidebar-originals = 本地副本
local-sidebar-proxies = 本地代理文件

local-synced = { $count ->
   *[other] 已将 { $count } 项更改写入原图。
}
local-sync-failed = { $count ->
   *[other] { $count } 项更改无法写入，仍在等待。
}

local-conflict-title = 离线期间 { $file } 发生了更改。要保留哪个{ $what }？
local-conflict-yours = 你的：{ $text }
local-conflict-theirs = 磁盘上：{ $text }
local-conflict-later = 取消可稍后再决定。不会覆盖任何内容。
local-keep-mine = 保留你的
local-keep-theirs = 保留文件的
local-what-marks = 评分
local-what-keywords = 关键词
local-what-fields = IPTC 字段
local-what-edit = 编辑
local-stars = { $count ->
    [0] 无星级
   *[other] { $count } 星
}
local-label = { $label ->
    [1] 红色
    [2] 黄色
    [3] 绿色
   *[4] 蓝色
}
local-pick = 选用
local-reject = 已排除
local-no-keywords = 无关键词
local-changed-elsewhere = 已在别处更改
local-photo-changed = 照片本身已更改
local-edit-here = 在此编辑，{ $when }
local-edit-elsewhere = 在别处编辑，{ $when }
local-edit-none = 无编辑

local-export-ask = { $count ->
   *[other] { $count } 张照片已离线。要从代理文件导出吗？
}
local-export-detail = 代理文件长边为 2560 像素。原图才有完整尺寸。
local-export-proxy = 从代理文件导出
local-export-wait = 等待原图
local-export-waiting = 原图恢复后开始导出。

local-panel-title = 本地副本
local-panel-used = 已使用 { $used }，共 { $limit }。
local-panel-originals = { $count ->
   *[other] { $count } 个原图
}
local-panel-proxies = { $count ->
   *[other] { $count } 张照片，RAW 为代理文件
}
local-panel-remove = 移除…
local-panel-footer = 存放在 { $folder }。原图永远不会更改。
local-panel-keywords = 离线 本地副本 代理文件 空间
