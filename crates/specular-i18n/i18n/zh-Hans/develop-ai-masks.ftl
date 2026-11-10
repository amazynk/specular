# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = 主体
develop-mask-kind-sky = 天空
develop-mask-kind-object = 点击的物体
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] 正在查找主体…
        [sky] 正在查找天空…
       *[object] 正在查找物体…
    }
develop-ai-ready = 完成
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = 点数：{ $n }
develop-ai-click-help =
    单击物体。
    Alt+单击不属于它的部分。
    拖动画框。
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    需要 { $model } 附加组件。
    在此之前不遮罩任何内容。
develop-ai-failed = 无法生成此蒙版：{ $error }
develop-ai-failed-worker = AI 蒙版进程已停止。
develop-ai-open-settings = 打开本地 AI…
develop-ai-clear = 清除点
develop-ai-too-many = 最多 { $n } 个点（一个框算两个）。
develop-ai-box-set = 已设置框
develop-ai-point-added = 已添加点
develop-ai-point-removed = 已在物体外添加点
develop-ai-cleared = 已清除点
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = 点 { $n }，在物体上
develop-ai-point-off = 点 { $n }，在物体外

## Settings › Local AI: the Masks feature
settings-ai-masks = AI 蒙版
settings-ai-masks-help = 冲印中的主体、天空和物体蒙版。物体模型随此功能提供；主体和天空是附加组件。
settings-ai-masks-size = 模型
settings-ai-masks-size-help = EfficientSAM Ti 更快。EfficientSAM S 更贴合边缘。
settings-ai-masks-size-recommended = 默认
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = 大
