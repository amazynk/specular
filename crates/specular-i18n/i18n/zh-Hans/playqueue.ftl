# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = 队列、重复和随机播放
# machine-drafted, needs native review
palette-next-track = 下一项
# machine-drafted, needs native review
palette-next-track-help = 播放队列中的下一项；没有队列时播放文件夹中同类的下一个文件。
# machine-drafted, needs native review
palette-prev-track = 上一项
# machine-drafted, needs native review
palette-prev-track-help = 播放上一项。超过 3 秒时从头重播当前项。
# machine-drafted, needs native review
palette-cycle-repeat = 重复
# machine-drafted, needs native review
palette-cycle-repeat-help = 重复：关闭、全部或单项。
# machine-drafted, needs native review
palette-shuffle = 随机播放
# machine-drafted, needs native review
palette-shuffle-help = 按随机顺序播放，每轮每项播放一次。
# machine-drafted, needs native review
palette-play-queue = 播放队列
# machine-drafted, needs native review
palette-play-queue-help = 显示接下来播放的内容：重新排序、移除、保存。
# machine-drafted, needs native review
palette-queue-play-next = 接下来播放
# machine-drafted, needs native review
palette-queue-play-next-help = 在当前项之后播放该文件（在画廊中为所选项）。
# machine-drafted, needs native review
palette-queue-add = 添加到队列
# machine-drafted, needs native review
palette-queue-add-help = 将该文件（在画廊中为所选项）添加到队列末尾。
# machine-drafted, needs native review
palette-save-queue = 将队列存为播放列表…
# machine-drafted, needs native review
palette-save-queue-help = 将队列存为 M3U8 或 XSPF 播放列表。
# machine-drafted, needs native review
transport-prev-track = 上一项
# machine-drafted, needs native review
transport-next-track = 下一项
# machine-drafted, needs native review
queue-title = 播放队列
# machine-drafted, needs native review
queue-summary = { $count ->
   *[other] { $count } 项
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
   *[other] { $count } 项
} · { $time }
# machine-drafted, needs native review
queue-empty = 队列为空。
    可用“接下来播放”或“添加到队列”添加声音文件和视频，或打开一个播放列表。
# machine-drafted, needs native review
queue-empty-short = 队列为空。
# machine-drafted, needs native review
queue-playing = 正在播放
# machine-drafted, needs native review
queue-repeat = 重复
# machine-drafted, needs native review
queue-repeat-off = 重复：关闭
# machine-drafted, needs native review
queue-repeat-all = 重复：全部
# machine-drafted, needs native review
queue-repeat-one = 重复：单项
# machine-drafted, needs native review
queue-shuffle = 随机
# machine-drafted, needs native review
queue-shuffle-on = 随机：开启
# machine-drafted, needs native review
queue-shuffle-off = 随机：关闭
# machine-drafted, needs native review
queue-save = 存为播放列表…
# machine-drafted, needs native review
queue-save-tip = 将队列存为 M3U8 或 XSPF 播放列表
# machine-drafted, needs native review
queue-clear = 清空…
# machine-drafted, needs native review
queue-clear-tip = 清空队列。文件仍保留在磁盘上。
# machine-drafted, needs native review
queue-remove = 从队列中移除（文件仍保留在磁盘上）
# machine-drafted, needs native review
queue-network-item = 网络项目，不会播放：{ $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = 没有下一项。
# machine-drafted, needs native review
queue-at-start = 没有上一项。
# machine-drafted, needs native review
queue-missing = { $name } 已不存在，已跳过。
# machine-drafted, needs native review
queue-network-not-played = 暂不播放网络项目。
# machine-drafted, needs native review
queue-nothing-to-add = 没有可添加的内容。只有声音文件、视频和播放列表能加入队列。
# machine-drafted, needs native review
queue-added = { $count ->
   *[other] 已将 { $count } 项添加到队列。
}
# machine-drafted, needs native review
queue-added-next = { $count ->
   *[other] 接下来将播放 { $count } 项。
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }：{ $count ->
   *[other] 队列中有 { $count } 项。
}
# machine-drafted, needs native review
queue-playlist-nothing = { $name } 中没有能在此播放的内容。队列会列出其中的项目。
# machine-drafted, needs native review
queue-playlist-failed = 无法打开 { $name }：{ $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = 无法读取该文件。
# machine-drafted, needs native review
queue-playlist-damaged = 这不是 Specular 能读取的播放列表。
# machine-drafted, needs native review
queue-clear-ask = { $count ->
   *[other] 清空队列（{ $count } 项）？
}
# machine-drafted, needs native review
queue-clear-detail = 只清空列表。文件仍保留在磁盘上。
# machine-drafted, needs native review
queue-clear-ok = 清空队列
# machine-drafted, needs native review
queue-cleared = 队列已清空。
# machine-drafted, needs native review
queue-save-title = 将队列存为播放列表
# machine-drafted, needs native review
queue-default-name = 队列
# machine-drafted, needs native review
queue-format-m3u8 = M3U8 播放列表
# machine-drafted, needs native review
queue-format-xspf = XSPF 播放列表
# machine-drafted, needs native review
queue-saved = 已保存 { $name }。
# machine-drafted, needs native review
queue-save-failed = 无法保存 { $name }：{ $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = 播放下一个声音文件
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = 声音文件结束时，播放文件夹中的下一个声音文件。
# machine-drafted, needs native review
settings-viewing-play-repeat = 重复
# machine-drafted, needs native review
settings-viewing-play-repeat-help = 适用于声音文件、视频和队列。
# machine-drafted, needs native review
settings-viewing-play-repeat-off = 关闭
# machine-drafted, needs native review
settings-viewing-play-repeat-all = 全部
# machine-drafted, needs native review
settings-viewing-play-repeat-one = 单项
# machine-drafted, needs native review
settings-viewing-play-shuffle = 随机播放
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = 按随机顺序播放，每轮每项播放一次。
