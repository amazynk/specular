# machine-drafted, needs native review
# Specular — Develop's AI masks: subject, sky and object-at-click rows, and the
# Masks feature in Settings › Local AI.

develop-mask-kind-subject = 被写体
develop-mask-kind-sky = 空
develop-mask-kind-object = クリックした物体
# An AI mask row while its network runs. $kind: subject, sky, object.
develop-ai-computing = { $kind ->
        [subject] 被写体を探しています…
        [sky] 空を探しています…
       *[object] 物体を探しています…
    }
develop-ai-ready = 完了
# An object row with its points ($n; a box counts as two).
develop-ai-object-clicks = 点: { $n }
develop-ai-click-help =
    物体をクリックします。
    含めない部分は Alt+クリック。
    ドラッグで枠。
# The row's model is not installed. $model is its name ("IS-Net").
develop-ai-needs =
    { $model } アドオンが必要です。
    それまでは何もマスクしません。
develop-ai-failed = このマスクを作れませんでした: { $error }
develop-ai-failed-worker = AI マスクの処理が停止しました。
develop-ai-open-settings = ローカル AI を開く…
develop-ai-clear = 点を消去
develop-ai-too-many = 最大 { $n } 点（枠は 2 点と数えます）。
develop-ai-box-set = 枠を設定しました
develop-ai-point-added = 点を追加しました
develop-ai-point-removed = 物体の外に点を追加しました
develop-ai-cleared = 点を消去しました
# Screen-reader names of the points on the picture. $n counts from 1.
develop-ai-point-on = 点 { $n }（物体上）
develop-ai-point-off = 点 { $n }（物体外）

## Settings › Local AI: the Masks feature
settings-ai-masks = AI マスク
settings-ai-masks-help = 現像の被写体・空・物体マスク。物体モデルはこの機能に含まれ、被写体と空はアドオンです。
settings-ai-masks-size = モデル
settings-ai-masks-size-help = EfficientSAM Ti は高速です。EfficientSAM S は輪郭により正確に沿います。
settings-ai-masks-size-recommended = デフォルト
settings-ai-masks-size-small = EfficientSAM Ti
settings-ai-masks-size-standard = EfficientSAM S
settings-ai-masks-size-large = 大
