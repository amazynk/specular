# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Japanese. Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = ローカル AI
settings-ai-pack = ローカル AI パック
# machine-drafted, needs native review
settings-ai-pack-help = このコンピューター上だけで動作するオプション機能です。見つけたものが外部に送られることはありません。機能ごとにモデルを選んでください。選択するまで何もダウンロードされません。
# machine-drafted, needs native review
settings-ai-install-from-file = ファイルからインストール…
settings-ai-install-from-file-help = このコンピューターにコピーしたモデルファイルをインストールします。インターネットのないマシン向けです。各ファイルは { -app-name } に同梱の一覧と照合されます。
settings-ai-install-from-folder = フォルダーからインストール…
settings-ai-install-from-folder-help = フォルダー内のすべてのモデルファイルに対して同じことを行います。
settings-ai-search = 説明で検索
settings-ai-search-help = 「浜辺の犬」のように説明して写真を探し、似たショットをより的確にまとめます。
settings-ai-text = 画像内のテキスト
settings-ai-text-help = 写真やスキャンのテキストを読み取り、text: 検索で見つかるようにします。
settings-ai-culling = 選別アシスト
# machine-drafted, needs native review
settings-ai-culling-help = 目つぶり、ブレ、露出の失敗に印を付け、連写の中からベストショットを選ぶ手助けをします。
settings-ai-people = 人物
# machine-drafted, needs native review
settings-ai-people-help = 顔を見つけて人物ごとにまとめ、people: 検索に使います。人物へのまとめは、ライブラリごとにオンにするまでオフです。顔データは人物ごとにも、まとめて全部も削除できます。

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] このビルドで使えるモデルはありません。
        [installed] インストール済み
        [indexing] インストール済み · { $total } 枚中 { $done } 枚をインデックス済み
        [downloading] ダウンロード中… { $percent }
        [failed] ダウンロードに失敗しました
       *[available] 未インストール
    }
ai-removing = 削除しています…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] ダウンロード
        [cancel] キャンセル
        [remove] 削除
        [forget] 忘れる
        [faces] すべての顔データを削除
       *[pack] パックを削除
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] すべての顔データを削除しますか？見つかったすべての顔と人物が削除されます。写真はそのまま残ります。
       *[pack] ローカル AI パック、そのモデル、見つけたすべてのデータを削除しますか？写真は変更されません。
    }

## Gallery search
gallery-query-needs-ai = { $field }: ローカル AI パックが必要です（設定 › ローカル AI）
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: ローカル AI パックはまだこの検索に対応していません

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = 顔を表示中
# machine-drafted, needs native review
faces-hidden = 顔を非表示
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = 目つぶり
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = 目つぶり
# machine-drafted, needs native review
cull-flag-eyes-closed-some = 目つぶり：{ $faces } 人中 { $closed } 人
# machine-drafted, needs native review
cull-flag-blurred = ぼやけ
# machine-drafted, needs native review
cull-flag-dark = 暗すぎ
# machine-drafted, needs native review
cull-flag-bright = 明るすぎ
# machine-drafted, needs native review
cull-flag-clipped = 白飛びまたは黒つぶれ
# machine-drafted, needs native review
# The People view.
people-title = 人物
# machine-drafted, needs native review
people-open = 人物
# machine-drafted, needs native review
people-done = 完了
# machine-drafted, needs native review
people-back = 戻る
# machine-drafted, needs native review
people-show-photos = 写真を表示
# machine-drafted, needs native review
people-name = 名前を付ける…
# machine-drafted, needs native review
people-name-prompt = この人物に名前を付ける
# machine-drafted, needs native review
people-faces-open = 顔
# machine-drafted, needs native review
people-faces-of = { $name } の顔
# machine-drafted, needs native review
people-merge = 統合
# machine-drafted, needs native review
people-merged = { $count } 人を統合しました
# machine-drafted, needs native review
people-merge-ask = { $from } を { $into } に統合しますか?
# machine-drafted, needs native review
people-merge-ask-many = { $count } 人を { $into } に統合しますか？
# machine-drafted, needs native review
people-merge-ask-detail = 顔は { $into } のものになります。ディスク上の写真は変わりません。
# machine-drafted, needs native review
people-not-this = この人物ではない
# machine-drafted, needs native review
people-face-removed = この人物から顔を外しました。写真はそのまま残ります。
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = 人物 #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
       *[other] { $count } 個の顔
    }
# machine-drafted, needs native review
people-count = { $count ->
       *[other] { $count } 人
    }
# machine-drafted, needs native review
people-face-a11y = 顔 { $index }
# machine-drafted, needs native review
people-loading = 読み込み中…
# machine-drafted, needs native review
people-not-installed = 人物はインストールされていません。設定 › ローカル AI でダウンロードしてください。
# machine-drafted, needs native review
people-grouping-off = このライブラリでは顔のまとめがオフです。設定 › ローカル AI で「顔を人物ごとにまとめる」をオンにしてください。
# machine-drafted, needs native review
people-none = まだ人物が見つかっていません。

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = モデル
settings-ai-search-size-help = SigLIP 2 B/16 は写真を最もよく見つけ、多くの言語に対応します。B/32 はインデックス作成が約 4 倍速く、遅めのグラフィックカード向けです。
settings-ai-search-size-recommended = 既定
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = モデル
settings-ai-text-size-help = PP-OCRv6 small はより正確に読み取り、tiny はより高速です。
settings-ai-text-size-recommended = 既定
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = 大
# machine-drafted, needs native review
settings-ai-culling-size = モデル
settings-ai-culling-size-help = 閉じた目を見つけるモデルです。
settings-ai-culling-size-recommended = 既定
settings-ai-culling-size-small = 小
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = 大
# machine-drafted, needs native review
settings-ai-people-size = モデル
settings-ai-people-size-help = SFace は小型です。AuraFace はより大きく、公開元が商用利用可能なライセンスと説明するデータで学習されています。
settings-ai-people-size-recommended = 既定
settings-ai-people-size-small = 小
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] 小
        [large] 大
       *[standard] 標準
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = 既定（{ $size }）
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = お使いの { $gpu } での既定: { $size }
ai-recommended = このコンピューターでの既定: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = このコンピューターのメモリに収まるサイズがありません。
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] 実行先：GPU（{ $gpu }）
        [gpu-unnamed] 実行先：GPU
        [no-gpu] 実行先：CPU（GPU が見つかりません）
        [differed] 実行先：CPU — GPU の結果が CPU と一致しませんでした
        [failed] 実行先：CPU — モデルが GPU で失敗しました
        [setting] 実行先：CPU（処理が「CPU のみ」に設定されています）
        [checking] 実行先：CPU — GPU と照合中
       *[cpu] 実行先：CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] ディスク上 { $bytes }
       *[download] ダウンロード { $bytes }
    } · メモリ { $memory }{ $gpu ->
        [yes] { " · GPU が必要" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] GPU が必要ですが、このコンピューターには使用できる GPU がありません。
       *[memory] { $need } のメモリが必要で、このコンピューターで Specular が AI に許可している { $have } を超えています。
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = 使用中は { $current } で、{ $chosen } ではありません。
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] 既定に戻す
        [anyway] このまま使用
       *[use] このサイズを使用
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = 選択済みですが、まだダウンロードされていません。{ $current } を引き続き使用します。
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] 使用中
        [downloaded] ダウンロード済み（未使用）
        [missing] 未ダウンロード
        [previous] 新しいサイズでライブラリのインデックスが完了するまで使用中
        [target] ダウンロード中…
        [failed] ダウンロードに失敗しました
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = ライセンスを表示
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: { $size } に切り替えますか？ライブラリのインデックスが再作成されます。それまでは現在のサイズが使われます。
       *[other] { $feature }: { $size } に切り替えますか？{ $download } をダウンロードし、ライブラリのインデックスを再作成します。それまでは現在のサイズが使われます。
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: それでも { $size } を使用しますか？このコンピューターでは動作しないか、非常に遅くなる可能性があります。
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: このコンピューターには { $size } の方が適しています。切り替えますか？{ $download ->
        [none] ライブラリのインデックスが再作成されます。
       *[other] { $download } をダウンロードし、ライブラリのインデックスを再作成します。
    }
ai-suggest-toast = ローカル AI: このコンピューターにより適したモデルのサイズがあります（設定 › ローカル AI）。
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = { $size } に切り替え中 · { $total } 枚中 { $done } 枚をインデックス済み

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = 今後のバージョンで対応予定
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] 多言語
        [english] 英語のみ
        [cjk-latin] 中国語、日本語、ラテン文字
        [zh-latin] 中国語、ラテン文字
        [arabic] アラビア語
        [hebrew] ヘブライ語
        [korean] 韓国語
        [cyrillic] キリル文字
        [greek] ギリシャ語
        [thai] タイ語
        [devanagari] デーヴァナーガリー
        [tamil] タミル語
        [telugu] テルグ語
        [eslav] ロシア語、ウクライナ語、ベラルーシ語
        [latin] ラテン文字（多くのヨーロッパ言語）
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] 高速、遅めのグラフィックカード向け
        [closed-eyes] 閉じた目を検出
        [small-faces] 小さな顔に強い
        [no-translate] 英語に翻訳できません
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] 学習データ: { $source }
        [research] 学習データ: { $source }（研究用途のみのライセンス）
       *[unstated] { $named ->
            [no] 学習データ: 名称なし、条件の記載なし
           *[yes] 学習データ: { $source }、条件の記載なし
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = { $host } からダウンロード
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = 顔を人物ごとにまとめる
ai-grouping-help = このライブラリでオンにするまでオフです。オフにするとまとめるのを停止します。まとめ済みの顔は削除するまで残ります。
ai-grouping-state = { $on ->
        [yes] このライブラリでオン
       *[no] オフ
    }
ai-grouping-button = { $on ->
        [yes] オフにする
       *[no] オンにする
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = 先に { $feature } をダウンロードしてください。
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = { $name } を削除しますか？
# machine-drafted, needs native review
ai-addon-remove-detail = モデルファイルが削除されます。写真とすでに見つかったテキストはそのまま残ります。
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = このライブラリの写真内のテキストを読み取る
# machine-drafted, needs native review
ai-text-indexing-help = このライブラリでオンにするまでオフです。オンにすると、ライブラリの検索で text: を使い、写真に写っている語句から写真を探せます。
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] このライブラリでオン
       *[no] オフ
    }

## ファイルからインストール

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
       *[other] モデルファイルを { $count } 件コピーしました。「ダウンロード」を押してインストールしてください。
    }
ai-sideload-none = モデルファイルが見つかりません。
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } は { -app-name } が認識するモデルではありません。コピーされませんでした。
ai-sideload-failed = { $file } をコピーできませんでした: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = 未検証のモデルを使用しますか？
ai-unverified-detail = { $file } は { -app-name } に同梱のモデル一覧にないため、検証できません。{ $model } として使用されます。
ai-unverified-use = 未検証のモデルを使用

# machine-drafted, needs native review
ai-failed-internal = { -app-name } の内部で問題が発生しました。詳細はログを確認してください。
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] 切り替える
       *[keep] 現在のサイズのままにする
    }
