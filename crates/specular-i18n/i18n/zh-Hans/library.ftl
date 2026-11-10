# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = 图库

## Top bar
library-title = 图库
library-locations-button = 位置

## Locations sidebar
library-locations = 位置
library-all = 所有位置
library-add = 添加位置…
library-remove = 从图库中移除 { $location }
library-offline = 离线
library-location-pending = 待索引 { $count } 个
library-a11y-location = { $location }, { $count ->
   *[other] { $count } 张图片
}
library-a11y-location-offline = { $location }, 离线, { $count ->
   *[other] { $count } 张图片
}

## Indexing progress
library-walking = 已找到图片：{ $count } · { $location }
# machine-drafted, needs native review
library-walking-count = 已找到图片：{ $count }
library-reading = 正在索引… { $done } / { $total }
library-held = 浏览时索引会暂停
library-up-to-date = 已是最新
# machine-drafted, needs native review
library-stopped = 索引已停止
library-stopped-pending = 索引已停止 · 待索引 { $count } 个
library-resume = 继续
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = 索引已停止：{ $reason }
library-reason-stalled = 已有 { $minutes ->
   *[other] { $minutes } 分钟
}没有任何进展。某个文件或驱动器可能没有响应。
library-retry = 重试
library-retry-description = 重新开始索引。已索引的内容仍可搜索。
library-save-diagnostics = 保存诊断包…
library-stop-description = 停止索引。已索引的内容仍可搜索；“继续”或“重新扫描”（F5）会接着进行。
library-resume-description = 重新查看所有位置，并索引剩余的内容。
library-opening = 正在打开图库…
library-searching = 正在搜索…

## Empty and error states
library-empty = 还没有位置。添加一个文件夹：其中及子文件夹中的图片会在此以一个网格显示。
library-empty-offline = 这些位置处于离线状态。它们恢复后图片会再次显示。
library-indexing = 正在查找图片…
# machine-drafted, needs native review
library-remove-confirm = 要从图库中移除“{ $location }”吗？
library-remove-confirm-detail = 你的照片仍保留在磁盘上，评分、标签和关键词也都保留。图库会忘记在那里建立的索引；再次添加该文件夹会重新建立索引。
library-remove-confirm-ok = 移除
library-indexing-in = 正在 { $location } 中查找图片… 目前已找到 { $count } 个
# machine-drafted, needs native review
library-empty-stopped = 在读取这些图片之前索引已被停止。“继续”会从那里接着进行。
library-unavailable = 无法打开图库：{ $reason }
library-read-only = 此图库由较新版本的 Specular 写入：可以搜索，但不能更新。

## Screen readers
library-a11y-list = 图库, { $count ->
   *[other] { $count } 张图片
}
library-a11y-added = 已将 { $location } 添加到图库
library-a11y-removed = 已从图库中移除 { $location }

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = 添加“{ $folder }”中的全部内容？
library-home-confirm-detail = 其中有程序文件、下载内容和缓存，不只是照片。索引会更久，图库里还会出现大量图标和其他图片。
library-home-confirm-pictures = “图片”文件夹里只有你的图片。
library-home-confirm-use-pictures = 添加“图片”
library-home-confirm-keep = 添加主文件夹

## Folder watches
# machine-drafted, needs native review
library-unwatched = 部分文件夹未受监视
library-unwatched-detail = { $minutes ->
   *[other] 更改会在 { $minutes } min 内显示
}
library-unwatched-description = Specular 的文件夹监视数量已用完。打开如何提高上限的帮助。

## Settings → Library
# machine-drafted, needs native review
settings-section-library = 图库
settings-library-skip-folders = 跳过名为以下名称的文件夹
settings-library-skip-folders-help = 图库不会查看使用这些名称的文件夹，以逗号分隔。隐藏文件夹始终会被跳过。
settings-library-skip-marked = 跳过包含以下内容的文件夹
settings-library-skip-marked-help = 也不会查看包含以下任一项的文件夹：.git 表示代码检出，pyvenv.cfg 表示 Python 环境，CACHEDIR.TAG 表示缓存。位置本身的文件夹不会被跳过。
settings-library-show-timeline = 时间轴
settings-library-show-timeline-help = 在图库网格上方显示每年、每月或每天的照片数量。
