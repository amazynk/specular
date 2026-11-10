# machine-drafted, needs native review
# Specular — 后台任务：后台有任务运行时显示的指示器及其打开的面板。简体中文。

## 面板

work-title = 后台任务
work-show = 显示后台任务
work-empty = 后台没有正在运行的任务。

## 任务

work-kind-library = 照片图库
work-kind-ai-search = AI 搜索
work-kind-faces = 人物
work-kind-text = 照片中的文字
work-kind-culling = 筛选评分
work-kind-download = AI 模型下载
work-kind-similar = 相似照片

## 步骤

work-step-finding = 正在查找照片
work-step-reading = 正在读取照片信息
work-step-signatures = 正在比较照片
work-step-grouping = 正在归组相似照片
work-step-text-index = 正在建立搜索索引
work-step-embedding = 正在为 AI 搜索索引照片
work-step-switching = 正在为新的模型大小重新索引
work-step-faces = 正在查找人脸
work-step-ocr = 正在读取照片中的文字
work-step-scoring = 正在评估清晰度和曝光
work-step-downloading = 正在下载
work-step-focus = 正在检查清晰度
work-step-item = { $step } · { $item }

## 进度

work-count-photos = { $done } / { $total } { $count ->
   *[other] 张照片
}
work-count = { $done } / { $total }
work-count-left = { $count } · 还剩 { $left }
work-so-far-photos = 目前已处理 { $done } { $count ->
   *[other] 张照片
}
work-so-far = 目前已处理 { $done }

## 剩余时间

work-eta-estimating = 正在估算…
work-eta-seconds = 大约还剩 { $n ->
   *[other] { $n } 秒
}
work-eta-minutes = 大约还剩 { $n ->
   *[other] { $n } 分钟
}
work-eta-hours = 大约还剩 { $h ->
   *[other] { $h } 小时
}
work-eta-hours-minutes = 大约还剩 { $h ->
   *[other] { $h } 小时
} { $m ->
   *[other] { $m } 分钟
}

## 任务等待的原因

work-paused-browsing = 浏览时已暂停
work-paused-battery = 使用电池供电时已暂停
work-paused-low-power = 省电模式下已暂停
work-paused-heat = 电脑过热，已暂停
work-paused-memory = 内存不足，已暂停
work-paused-user = 已暂停

## 接下来

work-then = 然后：{ $steps }

## 按钮

work-pause = 暂停
work-resume = 继续
work-settings = 设置…

## 指示器

work-chip = { $job } · { $done } / { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = 后台任务：{ $status }

## 播报

work-started = { $job }已在后台开始
work-finished = { $job }已完成
