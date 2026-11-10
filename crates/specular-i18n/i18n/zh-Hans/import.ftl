# machine-drafted, needs native review
# Specular — 从相机、手机和存储卡导入。简体中文。
# 存储卡只会被读取；只有在你要求时，且在副本经过校验后，才会从卡上删除文件。

palette-import = 导入照片…
disabled-no-import-source = 未连接存储卡、相机或手机

## 设置 → 导入
settings-section-import = 导入
settings-import-on-card = 插入存储卡时
settings-import-on-card-help = 连接带有照片的存储卡、相机或手机时要执行的操作。
settings-import-on-card-ask = 询问
settings-import-on-card-open = 打开导入
settings-import-on-card-nothing = 不执行任何操作
settings-import-destination = 导入到
settings-import-destination-help = 存放导入的照片和视频的文件夹。未设置时存入“图片”文件夹。
settings-import-destination-unset = “图片”文件夹
settings-import-folder-pattern = 文件夹
settings-import-folder-pattern-help = 在其中创建的文件夹：{"{"}year{"}"}、{"{"}month{"}"}、{"{"}day{"}"}、{"{"}camera{"}"} 和 {"{"}event{"}"}，用 / 分隔。
settings-import-rename-pattern = 重命名
settings-import-rename-pattern-help = 新文件名，写法与“转换”相同：{"{"}name{"}"}、{"{"}n:03{"}"}、{"{"}date:%Y%m%d{"}"}、{"{"}camera{"}"}、{"{"}event{"}"}。留空则保留相机的文件名。
settings-import-keep-pairs = RAW 和 JPEG 放在一起
settings-import-keep-pairs-help = RAW 文件和同时拍摄的 JPEG 放入同一文件夹并使用同一名称，重命名后也是如此。
settings-import-low-res-videos = 导入低分辨率视频副本
settings-import-low-res-videos-help = 有些相机会为每段视频录制一个小的预览副本（.LRV）。关闭时，它们留在卡上。
settings-import-eject = 完成后弹出
settings-import-eject-help = 全部复制完成后弹出存储卡，以便取出。

## 插入存储卡时的提示
import-offer = 从 { $name } 导入？
import-offer-button = 导入

## 导入视图
import-title = 导入
import-from = 来自 { $name }
import-source-next = 下一个来源
import-close = 关闭
import-scanning = 正在查找新照片… { $done }/{ $total }
import-scanning-start = 正在查找新照片…
import-none-new = { $name } 上的所有内容此前都已导入。
import-empty = { $name } 上没有照片或视频。
import-no-source = 未连接存储卡、相机或手机。
import-summary = { $count ->
   *[other] { $count } 个新项目
} · { $size }
import-already = { $count } 个此前已导入
import-unreadable = { $count ->
   *[other] { $count } 个文件无法读取
}
import-selected = 已选择 { $count }/{ $total }
import-select-all = 全选
import-select-none = 全不选
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = 视频

import-destination = 导入到
import-choose = 选择…
import-folder-pattern = 文件夹
import-event = 事件
import-event-hint = 可选，用于 {"{"}event{"}"}
import-rename = 重命名
import-rename-hint = 留空则保留相机的文件名
import-preview = 第一个文件：{ $path }
import-pattern-error = { $error }
import-keep-pairs = RAW 和 JPEG 放在一起
import-eject = 完成后弹出
import-delete = 导入后从存储卡删除
import-free = 需复制 { $size } · 可用空间 { $free }
import-free-unknown = 需复制 { $size }
import-no-space = { $folder } 中的可用空间不足：此次导入需要 { $need }，但只有 { $free } 可用。请释放空间或选择其他文件夹。
import-start = { $count ->
   *[other] 导入 { $count } 个项目
}
import-nothing-selected = 选择要导入的照片

import-confirm-delete = { $count ->
   *[other] 复制并校验后，将从 { $name } 删除 { $count } 个文件。此操作无法撤销。
}
import-confirm-yes = 导入并删除
import-confirm-no = 返回

import-progress = 正在导入 { $done }/{ $total } · { $size }
import-progress-eta = 正在导入 { $done }/{ $total } · { $size } · { $time }
import-paused = 已暂停 · { $done }/{ $total }
import-pause = 暂停
import-resume = 继续
import-cancel = 取消
import-hide = 隐藏
import-stopping = 正在停止…
import-running = 已有导入正在进行。

import-done = { $count ->
   *[other] 已导入 { $count } 个文件
}到 { $folder }
import-done-existed = { $count } 个已存在
import-done-deleted = 已从存储卡删除 { $count } 个
import-done-failed = { $count ->
   *[other] { $count } 个文件无法导入
}
import-done-cancelled = 导入在 { $count } 个文件后取消。再次导入会从中断处继续。
import-done-nothing = 没有导入新内容。
import-ejected = 现在可以取出 { $name }。
import-eject-failed = 无法弹出 { $name }：{ $error }
import-failed-destination = 无法使用文件夹 { $folder }：{ $error }
import-ledger-failed = 无法读取已导入文件列表（{ $error }）；此前导入的文件可能会再次出现。

## 屏幕阅读器
import-a11y-group = 导入
import-a11y-tile = { $name }，{ $date }
import-a11y-tile-selected = { $name }，{ $date }，已选择
