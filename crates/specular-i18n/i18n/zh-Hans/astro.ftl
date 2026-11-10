# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = 叠加 DS9 区域…
palette-science-load-regions-help = 在图像上绘制 DS9 区域文件 (.reg) 中的形状。
palette-science-export-regions = 将区域保存为 DS9…
palette-science-export-regions-help = 将测量和叠加的区域保存为 DS9 区域文件。
palette-science-edit-regions = 将区域作为测量编辑
palette-science-edit-regions-help = 把叠加的区域转换为可移动、可修改的测量。
palette-science-clear-regions = 移除叠加的区域
palette-science-clear-regions-help = 从图像上移除 DS9 区域。
palette-science-plate-solve = 星图解算
palette-science-plate-solve-help = 用 ASTAP 或 astrometry.net 求出图像的天球坐标。再次运行可停止。
palette-science-save-plate-solution = 保存星图解算结果
palette-science-save-plate-solution-help = 将求得的天球坐标以 .wcs 文件保存在图像旁边。
palette-science-sky-survey = 巡天背景
palette-science-sky-survey-help = 在图像周围显示下一个巡天，或不显示。
palette-science-samp-send-image = 将图像发送到 SAMP 应用
palette-science-samp-send-image-help = 在 DS9、Aladin 及 SAMP 集线器上的其他应用中打开此图像。
palette-science-samp-send-position = 将位置发送到 SAMP 应用
palette-science-samp-send-position-help = 让其他应用指向指针下的天球位置。

disabled-samp-off = 请先连接到 SAMP 集线器（设置 › 集成）。

settings-integration-samp = 连接到 SAMP 集线器
settings-integration-samp-help = 与 DS9、TOPCAT、Aladin 等天文应用交换图像、表格和天球位置。只使用本机上的集线器。
settings-integration-astap-path = ASTAP 程序
settings-integration-astap-path-help = 未能自动找到时使用的 ASTAP 星图解算器。
settings-integration-solve-field-path = astrometry.net 程序
settings-integration-solve-field-path-help = 未能自动找到时使用的 astrometry.net 的 solve-field 程序。

astro-regions-filter = DS9 区域
astro-regions-loaded = { $name }：{ $count } 个区域。
astro-regions-refused = 有 { $count } 行未读取。第一处：第 { $line } 行，{ $reason }。
astro-regions-approximate = { $count } 个 panda 以内外边缘绘制。
astro-regions-none-here = { $name }：没有区域落在此图像上。
astro-regions-failed = 无法读取 { $name }：{ $reason }
astro-regions-cleared = 已移除区域。
astro-regions-nothing = 没有可保存的区域或测量。
astro-regions-saved = 已保存 { $name }。
astro-regions-skipped = 已略去 { $count } 个超出天球坐标的形状。
astro-regions-save-failed = 无法保存区域：{ $reason }
astro-regions-edited = { $count } 个区域已转为测量。
astro-regions-no-wcs = 天球区域需要带天球坐标的图像。

astro-solve-no-solver = 未找到星图解算器。请安装 ASTAP 或 astrometry.net，或在 设置 › 集成 中选择一个。
astro-solve-running = 正在用 { $solver } 解算… { $seconds } 秒
astro-solve-stop = 停止
astro-solve-stopped = 已停止星图解算。
astro-solve-done = 已解算：{ $ra }，{ $dec }，每像素 { $scale }。
astro-solve-failed = 星图解算失败：{ $reason }
astro-solve-not-local = 只能解算本机文件夹中的图像。
astro-solve-none = 此图像尚无星图解算结果。
astro-solve-saved = 已保存 { $name }。图像本身未改变。
astro-solve-sidecar = 天球坐标来自 { $name }。

astro-samp-connected = 已连接到 SAMP 集线器。
astro-samp-no-hub = SAMP：{ $reason }。集线器启动后将重试。
astro-samp-lost = SAMP 集线器已停止。有集线器启动后将重试。
astro-samp-sent = 已发送到 { $count } 个应用。
astro-samp-nobody = 集线器上没有应用接收此消息。
astro-samp-failed = SAMP：{ $reason }
astro-samp-refused = SAMP：{ $url } 不在本机上，因此不会打开。
astro-samp-pointed = 已指向 { $ra }，{ $dec }。
astro-samp-off-image = { $ra }，{ $dec } 不在此图像上。
astro-samp-no-table = SAMP：该表格未叠加在这里。
astro-samp-rows = 已高亮 { $count } 个星表源。

astro-survey-off = 巡天已关闭。
astro-survey-on = 巡天：{ $name }。{ $credit }
astro-survey-failed = 无法显示 { $name }：{ $reason }
astro-survey-credit = { $name }：{ $credit }

net-what-sky-survey = 来自 { $host } 的巡天图块
