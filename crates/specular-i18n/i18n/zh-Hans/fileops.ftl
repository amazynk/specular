# machine-drafted, needs native review
# 文件操作：重命名、批量重命名、复制和移动到文件夹、保存到文件的旋转、
# 撤销和重做、设为壁纸、共享、通过电子邮件发送、导出和导入设置。

## 命令面板、速查表和菜单

palette-undo = 撤销
palette-undo-help = 撤销对文件的最后一次更改：移到回收站、重命名、复制或移动、旋转。
palette-redo = 重做
palette-redo-help = 重新执行上一次撤销的操作。
palette-rename = 重命名…
palette-rename-help = 为文件取新名称；其附属文件和 RAW+JPEG 配对文件随之更改。
palette-batch-rename = 批量重命名…
palette-batch-rename-help = 按模式重命名所选文件或整个文件夹，带预览。
palette-copy-to-folder = 复制到文件夹…
palette-copy-to-folder-help = 将文件或所选项目复制到另一个文件夹。
palette-move-to-folder = 移动到文件夹…
palette-move-to-folder-help = 将文件或所选项目移动到另一个文件夹。
palette-rotate-file-left = 向左旋转文件
palette-rotate-file-left-help = 将文件本身向左旋转 90°，格式允许时无损。
palette-rotate-file-right = 向右旋转文件
palette-rotate-file-right-help = 将文件本身向右旋转 90°，格式允许时无损。
palette-set-wallpaper-help = 将图像设为桌面背景。
palette-share = 共享…
palette-share-help = 通过系统的共享菜单将文件发送到其他应用。
palette-send-by-email = 通过电子邮件发送…
palette-send-by-email-help = 在邮件应用中新建一封附带该文件的邮件。
# machine-drafted, needs native review
palette-share-with-zynk = 通过 Zynk 共享…
palette-share-with-zynk-help = 在 Zynk 应用中为该文件创建网页链接。
palette-send-with-zynk = 通过 Zynk 发送…
palette-send-with-zynk-help = 通过 Zynk 将该文件发送给联系人或您的某台设备。
palette-export-settings = 导出设置…
palette-export-settings-help = 将所有设置和快捷键保存到文件。
palette-import-settings = 导入设置…
palette-import-settings-help = 先显示将更改的内容，再从文件读取设置。

disabled-in-archive-file = 此页面位于压缩包内，没有自己的文件
disabled-not-rotatable = 此类文件无法记录旋转
disabled-not-on-this-desktop = 你的桌面不提供此功能

## 设置

settings-browsing-rotate-trim-edges = 裁掉 JPEG 边缘以无损旋转
settings-browsing-rotate-trim-edges-help = 尺寸不是 8 或 16 像素倍数的 JPEG，只有裁掉最后几个像素才能无损旋转。关闭时改为修改其方向标签，并每次询问是否裁剪。
settings-browsing-rotate-keep-modified = 旋转时保留修改日期
settings-browsing-rotate-keep-modified-help = 旋转后的文件保留其最后修改日期，按日期排序时位置不变。
settings-browsing-recent-folders = 最近的文件夹
settings-browsing-recent-folders-help = 最近用于复制和移动的文件夹。

## 对话框

fileop-cancel = 取消
rename-title = 重命名
rename-field-label = 名称
rename-button = 重命名
rename-sidecars-follow = 一并重命名：{ $files }
rename-extension-changed = 更改扩展名不会转换文件；其他应用可能无法打开它。
rename-problem-empty = 请输入名称。
rename-problem-invalid-char = 名称不能包含“{ $char }”。
rename-problem-reserved = “{ $name }”是 Windows 为设备保留的名称。
rename-problem-too-long = 名称太长；最多 255 字节。
rename-problem-dot = 不能使用“.”和“..”作为名称。
rename-problem-trailing = 名称不能以空格或句点结尾。
rename-problem-exists = 已有文件使用此名称。
rename-problem-read-only = 此文件夹无法更改。
rename-problem-duplicate = 会有两个文件得到此名称。

batch-rename-title =
    { $count ->
       *[other] 重命名 { $count } 个文件
    }
batch-rename-field-label = 模式
batch-rename-tokens = 可使用 { "{" }name{ "}" }、{ "{" }n:3{ "}" }（001、002、…）、{ "{" }date:%Y-%m-%d{ "}" }、{ "{" }camera{ "}" }、{ "{" }w{ "}" }×{ "{" }h{ "}" } 和 { "{" }ext{ "}" }。
batch-rename-summary =
    { $count ->
       *[other] 将重命名 { $count } 个文件
    }{ $problems ->
        [0] 。
       *[other] ；{ $problems } 个无法重命名，保持原样。
    }
batch-rename-reading = 正在读取日期和相机…
batch-rename-preview = 预览
batch-rename-unchanged = 名称不变
batch-rename-more =
    { $count ->
       *[other] 下方还有 { $count } 个
    }

transfer-copy-title-one = 将“{ $file }”复制到文件夹
transfer-copy-title =
    { $count ->
       *[other] 将 { $count } 个文件复制到文件夹
    }
transfer-move-title-one = 将“{ $file }”移动到文件夹
transfer-move-title =
    { $count ->
       *[other] 将 { $count } 个文件移动到文件夹
    }
transfer-recent = 最近的文件夹
transfer-folder = 文件夹
transfer-choose-folder = 选择文件夹…
transfer-copy-dialog = 复制到文件夹
transfer-move-dialog = 移动到文件夹
transfer-sidecars =
    { $count ->
       *[other] { $count } 个附属文件随之处理。
    }
transfer-taken =
    { $count ->
       *[other] { $folder } 中已有 { $count } 个名称被使用。
    }
transfer-when-taken = 名称已被使用时
transfer-skip = 跳过
transfer-keep-both = 保留两者
transfer-replace-option = 替换
transfer-copy = 复制
transfer-move = 移动
transfer-replace-title =
    { $count ->
       *[other] 替换 { $folder } 中的 { $count } 个文件？
    }
transfer-replace-note = 那里现有的文件将移到回收站。撤销可恢复。
transfer-replace = 替换
transfer-back = 返回
transfer-stop = 停止

settings-file-import-title = 导入设置
settings-file-import-dialog = 导入设置
settings-file-export-dialog = 导出设置
settings-file-import-changes = 更改
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = 无按键
settings-file-import-unknown = 不是此版本的设置；已略过。
settings-file-import-keymap = 快捷键
settings-file-import-nothing = 不会有任何更改：这些设置已在使用中。
settings-file-import-summary =
    { $count ->
       *[other] { $count } 项更改。可在设置中撤销导入。
    }
settings-file-import-secrets =
    { $count ->
       *[other] { $count } 个条目看似密码或密钥，绝不会导入。
    }
settings-file-import-commands =
    { $count ->
       *[other] 未导入 { $count } 个用户命令：命令会启动程序。如果信任，请自行复制到设置文件中。
    }
settings-file-import-button = 导入

## 结果

fileop-renamed = 已重命名为 { $file }
fileop-renamed-count =
    { $count ->
       *[other] 已重命名 { $count } 个文件
    }
fileop-copied =
    { $count ->
       *[other] 已将 { $count } 个文件复制到 { $folder }
    }
fileop-moved =
    { $count ->
       *[other] 已将 { $count } 个文件移动到 { $folder }
    }
fileop-skipped =
    { $count ->
       *[other] 已跳过 { $count } 个：那里已有同名文件。
    }
fileop-stopped = 已停止；已完成的部分保持不变。
fileop-failed-file =
    { $count ->
       *[other] 无法复制或移动 { $count } 个文件；第一个是 { $file }：{ $reason }
    }
fileop-progress-copy =
    { $count ->
       *[other] 正在复制 { $count } 个文件… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
       *[other] 正在移动 { $count } 个文件… { PERCENT($percent) }
    }
fileop-done-status = 完成
fileop-busy = 仍在处理上一个文件操作；请稍后再试。
fileop-nothing-to-redo = 没有可重做的操作
fileop-undone = 已撤销：{ $what }
fileop-redone = 已重做：{ $what }
fileop-kind-rename = 重命名
fileop-kind-batch-rename = 批量重命名
fileop-kind-move = 移动到文件夹
fileop-kind-copy = 复制到文件夹
fileop-kind-rotate = 旋转
rename-failed = 无法重命名：{ $reason }

undo-problem-changed = 无法撤销：{ $file } 此后已被更改，撤销会丢失该更改。
undo-problem-missing = 无法撤销：{ $file } 已不存在。
undo-problem-taken = 无法撤销：另一个文件现在名为 { $file }。
undo-problem-io = 无法撤销对 { $file } 的更改：{ $reason }

rotate-done-lossless = 已无损旋转 { $file }。
rotate-done-lossless-trimmed = 已无损旋转 { $file }；边缘已裁剪为 { $width } × { $height }。
rotate-done-tag = 已通过修改方向标签旋转 { $file }。
rotate-done-sidecar = 已将 { $file } 的旋转保存到 { $sidecar }；RAW 文件本身从不更改。
rotate-tag-unreadable = 无法读取足够的图像数据来旋转它。
rotate-tag-multi-picture = 它包含多张图像，旋转第一张会损坏其他图像。
rotate-tag-trailer = 图像后面还有数据，旋转会使其移位。
rotate-tag-not-aligned = 无损旋转像素会将其裁剪为 { $width } × { $height }。
rotate-trim-offer = 裁剪并旋转
rotate-problem-unsupported = { $file } 这类文件无法记录旋转。
rotate-problem-too-large = { $file } 太大，无法旋转。
rotate-problem-damaged = { $file } 存放方向信息的部分已损坏：{ $reason }
rotate-problem-io = 无法旋转 { $file }：{ $reason }

share-failed = 无法共享：{ $reason }
email-failed = 无法启动邮件应用：{ $reason }
# machine-drafted, needs native review
zynk-opening = 正在打开 Zynk…
zynk-failed = 无法连接 Zynk：{ $reason }
zynk-get-title-send = “通过 Zynk 发送”需要 Zynk 应用
zynk-get-title-share = “通过 Zynk 共享”需要 Zynk 应用
zynk-get-about = Zynk 是一款免费应用，支持 Mac、Windows、Linux、iPhone 和 Android。它能把照片、视频和整个文件夹直接发送到你的其他设备或发给其他人，不限大小。传输采用端到端加密，连接中断后会从中断处继续。对方没有 Zynk 时，它会生成一个网页链接，对方在浏览器中即可下载。
zynk-get-macos = 请从 Mac App Store 获取。安装后，{ -app-name } 会把你的文件直接交给它。
zynk-get-windows = 请从 Microsoft Store 获取。安装后，{ -app-name } 会把你的文件直接交给它。
zynk-get-linux = Linux 版 Zynk 是一个 AppImage，无需安装：下载后放到 ~/Applications 并设为可执行（在文件属性中，或使用 chmod +x）。{ -app-name } 会在那里找到它；Zynk 运行时，放在其他位置也能找到。
zynk-get = 获取 Zynk
zynk-send-old-windows = “通过 Zynk 发送”需要 Windows 10 版本 2004 或更高版本。在此 Windows 上可以使用“通过 Zynk 共享”。
settings-file-export-done = 设置已保存到 { $file }
settings-file-export-failed = 无法保存设置：{ $reason }
settings-file-import-unreadable = 无法读取 { $file }：{ $reason }
settings-file-import-done =
    { $count ->
       *[other] 已导入 { $count } 项更改
    }
