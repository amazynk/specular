# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Simplified Chinese. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = 本地 AI
settings-ai-pack = 本地 AI 包
# machine-drafted, needs native review
settings-ai-pack-help = 只在本机运行的可选功能。它们发现的任何内容都不会离开这台电脑。为每项功能选择一个模型；在你选择之前不会下载任何内容。
# machine-drafted, needs native review
settings-ai-install-from-file = 从文件安装…
settings-ai-install-from-file-help = 安装已复制到这台电脑的模型文件，适用于无法联网的机器。每个文件都会与 { -app-name } 自带的列表核对。
settings-ai-install-from-folder = 从文件夹安装…
settings-ai-install-from-folder-help = 同上，适用于文件夹中的所有模型文件。
settings-ai-search = 按描述搜索
settings-ai-search-help = 通过描述查找照片，例如“海滩上的狗”，并更好地归组相似的照片。
settings-ai-text = 图像中的文字
settings-ai-text-help = 识别照片和扫描件中的文字，以便 text: 搜索能找到它们。
settings-ai-culling = 筛选辅助
# machine-drafted, needs native review
settings-ai-culling-help = 标记闭眼、模糊和曝光不佳，帮助从连拍中挑出最佳照片。
settings-ai-people = 人物
# machine-drafted, needs native review
settings-ai-people-help = 查找人脸并按人物归组，用于 people: 搜索。在你为某个图库开启之前，归组保持关闭。可以删除某个人物或全部人物的人脸数据。

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] 此版本中没有可用的模型。
        [installed] 已安装
        [indexing] 已安装 · 已索引 { $done }/{ $total } 张图像
        [downloading] 正在下载… { $percent }
        [failed] 下载失败
       *[available] 未安装
    }
ai-removing = 正在移除…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] 下载
        [cancel] 取消
        [remove] 移除
        [forget] 忘记
        [faces] 删除所有人脸数据
       *[pack] 移除 AI 包
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] 要删除所有人脸数据吗？找到的所有人脸和人物都会被删除。你的照片会保留。
       *[pack] 要移除本地 AI 包、其模型以及它找到的所有内容吗？你的照片不会被更改。
    }

## Gallery search
gallery-query-needs-ai = { $field }：需要本地 AI 包（设置 › 本地 AI）
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }：本地 AI 包暂时还不能搜索此项

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = 已显示人脸
# machine-drafted, needs native review
faces-hidden = 已隐藏人脸
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = 闭眼
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = 闭眼
# machine-drafted, needs native review
cull-flag-eyes-closed-some = 闭眼：{ $faces } 张人脸中有 { $closed } 张
# machine-drafted, needs native review
cull-flag-blurred = 模糊
# machine-drafted, needs native review
cull-flag-dark = 过暗
# machine-drafted, needs native review
cull-flag-bright = 过亮
# machine-drafted, needs native review
cull-flag-clipped = 高光或阴影裁剪
# machine-drafted, needs native review
# The People view.
people-title = 人物
# machine-drafted, needs native review
people-open = 人物
# machine-drafted, needs native review
people-done = 完成
# machine-drafted, needs native review
people-back = 返回
# machine-drafted, needs native review
people-show-photos = 显示照片
# machine-drafted, needs native review
people-name = 命名…
# machine-drafted, needs native review
people-name-prompt = 为此人物命名
# machine-drafted, needs native review
people-faces-open = 人脸
# machine-drafted, needs native review
people-faces-of = { $name } 的人脸
# machine-drafted, needs native review
people-merge = 合并
# machine-drafted, needs native review
people-merged = 已合并 { $count } 个人物
# machine-drafted, needs native review
people-merge-ask = 将 { $from } 合并到 { $into }?
# machine-drafted, needs native review
people-merge-ask-many = 将 { $count } 人合并到 { $into }？
# machine-drafted, needs native review
people-merge-ask-detail = 其面孔将归入 { $into }。磁盘上的照片不会改变。
# machine-drafted, needs native review
people-not-this = 不是此人
# machine-drafted, needs native review
people-face-removed = 已从此人物中移除这张人脸。照片保留。
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = 人物 #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
       *[other] { $count } 张人脸
    }
# machine-drafted, needs native review
people-count = { $count ->
       *[other] { $count } 个人物
    }
# machine-drafted, needs native review
people-face-a11y = 人脸 { $index }
# machine-drafted, needs native review
people-loading = 正在加载…
# machine-drafted, needs native review
people-not-installed = 未安装“人物”。请在“设置 › 本地 AI”中下载。
# machine-drafted, needs native review
people-grouping-off = 此图库的人脸归组已关闭。请在“设置 › 本地 AI”中开启“将人脸归组为人物”。
# machine-drafted, needs native review
people-none = 尚未找到人物。

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = 模型
settings-ai-search-size-help = SigLIP 2 B/16 找照片最准，支持多种语言。B/32 索引速度约快四倍，适合较慢的显卡。
settings-ai-search-size-recommended = 默认
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = 模型
settings-ai-text-size-help = PP-OCRv6 small 识别更准确；tiny 更快。
settings-ai-text-size-recommended = 默认
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = 大
# machine-drafted, needs native review
settings-ai-culling-size = 模型
settings-ai-culling-size-help = 用于发现闭眼的模型。
settings-ai-culling-size-recommended = 默认
settings-ai-culling-size-small = 小
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = 大
# machine-drafted, needs native review
settings-ai-people-size = 模型
settings-ai-people-size-help = SFace 较小。AuraFace 较大，其训练数据被发布者称为已获商用许可。
settings-ai-people-size-recommended = 默认
settings-ai-people-size-small = 小
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] 小
        [large] 大
       *[standard] 标准
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = 默认（{ $size }）
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = 你的 { $gpu } 的默认模型：{ $size }
ai-recommended = 这台电脑的默认模型：{ $size }
# Nothing fits: no size can run here.
ai-recommended-none = 没有适合这台电脑内存的大小。
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] 运行于：GPU（{ $gpu }）
        [gpu-unnamed] 运行于：GPU
        [no-gpu] 运行于：CPU（未找到 GPU）
        [differed] 运行于：CPU — GPU 的结果与 CPU 不一致
        [failed] 运行于：CPU — 模型在 GPU 上失败
        [setting] 运行于：CPU（处理位置设置为“仅 CPU”）
        [checking] 运行于：CPU — 正在用它核对 GPU
       *[cpu] 运行于：CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] 磁盘占用 { $bytes }
       *[download] 下载 { $bytes }
    } · 内存 { $memory }{ $gpu ->
        [yes] { " · 需要 GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] 需要 GPU，而这台电脑没有可用的 GPU。
       *[memory] 需要 { $need } 内存，超过了 Specular 在这台电脑上允许 AI 使用的 { $have }。
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = 正在使用的是{ $current }，而不是{ $chosen }。
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] 恢复默认
        [anyway] 仍然使用
       *[use] 使用此大小
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = 已选择，但尚未下载：仍在使用{ $current }。
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] 使用中
        [downloaded] 已下载，未使用
        [missing] 未下载
        [previous] 使用中，直到新大小为你的图库建立索引
        [target] 正在下载…
        [failed] 下载失败
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = 查看许可证
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }：切换到{ $size }？你的图库将重新建立索引；在此之前仍使用当前大小。
       *[other] { $feature }：切换到{ $size }？这将下载 { $download } 并重新为你的图库建立索引；在此之前仍使用当前大小。
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }：仍然使用{ $size }吗？它可能无法在这台电脑上运行，或者运行得非常慢。
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }：{ $size }现在更适合这台电脑。要切换吗？{ $download ->
        [none] 你的图库将重新建立索引。
       *[other] 这将下载 { $download } 并重新为你的图库建立索引。
    }
ai-suggest-toast = 本地 AI：另一种模型大小更适合这台电脑（设置 › 本地 AI）。
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = 正在切换到{ $size } · 已索引 { $done }/{ $total } 张图像

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = 将在以后的版本中提供
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] 多种语言
        [english] 仅英语
        [cjk-latin] 中文、日文和拉丁字母
        [zh-latin] 中文和拉丁字母
        [arabic] 阿拉伯文
        [hebrew] 希伯来文
        [korean] 韩文
        [cyrillic] 西里尔字母
        [greek] 希腊文
        [thai] 泰文
        [devanagari] 天城文
        [tamil] 泰米尔文
        [telugu] 泰卢固文
        [eslav] 俄语、乌克兰语和白俄罗斯语
        [latin] 拉丁字母（多种欧洲语言）
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] 更快，适合较慢的显卡
        [closed-eyes] 可发现闭眼
        [small-faces] 更擅长小人脸
        [no-translate] 不能翻译成英语
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] 训练数据：{ $source }
        [research] 训练数据：{ $source }（仅限研究许可）
       *[unstated] { $named ->
            [no] 训练数据：未注明，条款未说明
           *[yes] 训练数据：{ $source }，条款未说明
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = 从 { $host } 下载
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = 将人脸归组为人物
ai-grouping-help = 在你为此图库开启之前保持关闭。关闭后停止归组；已归组的人脸会保留，直到你删除。
ai-grouping-state = { $on ->
        [yes] 已在此图库中开启
       *[no] 关闭
    }
ai-grouping-button = { $on ->
        [yes] 关闭
       *[no] 开启
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = 请先下载{ $feature }。
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = 要移除 { $name } 吗？
# machine-drafted, needs native review
ai-addon-remove-detail = 其模型文件会被删除。你的照片和已找到的文字会保留。
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = 识别此图库照片中的文字
# machine-drafted, needs native review
ai-text-indexing-help = 在你为此图库开启之前保持关闭。开启后，在图库搜索中使用 text: 可按照片中的文字查找照片。
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] 已在此图库中开启
       *[no] 关闭
    }

## 从文件安装

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
       *[other] 已复制 { $count } 个模型文件。按“下载”进行安装。
    }
ai-sideload-none = 未找到模型文件。
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } 不是 { -app-name } 认识的模型，未复制。
ai-sideload-failed = 无法复制 { $file }：{ $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = 使用未经验证的模型？
ai-unverified-detail = { $file } 不在 { -app-name } 自带的模型列表中，因此无法核对。它将用作 { $model }。
ai-unverified-use = 使用未经验证的模型

# machine-drafted, needs native review
ai-failed-internal = { -app-name } 内部出错。详情见日志。
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] 切换
       *[keep] 保留当前大小
    }
