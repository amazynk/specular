# machine-drafted, needs native review
# Photographer polish pass: Develop regions, crop rows, import choices, keyword rename and rules.

palette-develop-next-region = 次の領域
palette-develop-next-region-help = キーボードの操作先を進めます：画像、パネル、フィルムストリップ。
palette-develop-prev-region = 前の領域
palette-develop-prev-region-help = キーボードの操作先を戻します：フィルムストリップ、パネル、画像。
develop-region-picture = 画像
develop-region-panel = パネル

develop-crop-aspect-row = 縦横比
develop-crop-aspect-option-free = 自由
develop-crop-aspect-option-original = 元の比率
develop-crop-aspect-option-custom = カスタム
develop-crop-type-ratio = 比率を入力…
develop-crop-swap = 縦向きと横向きを入れ替え
develop-crop-turn-left = 左に回転
develop-crop-turn-right = 右に回転
develop-crop-flip-horizontal = 左右に反転
develop-crop-flip-vertical = 上下に反転
develop-crop-ratio-title = 切り抜きの縦横比
develop-crop-ratio-field = 幅:高さ
develop-crop-ratio-ok = 設定
develop-crop-ratio-bad = 5:4 のような比率か、1.85 のような数値を 1 つ入力してください。

import-template = メタデータテンプレート
import-style = 現像スタイル
import-choice-none = なし
import-choice-said = { $label }：{ $value }
import-keywords = キーワード
import-keywords-hint = 例：家族; 場所 > パリ
import-style-missing = スタイル { $name } が見つかりません。写真はスタイルなしで読み込まれました。

keywords-rename = 名前を変更…
keywords-move = 移動…
keywords-rename-of = { $keyword } の新しい名前
keywords-move-of = { $keyword } を入れる場所（空欄：最上位）
keywords-move-into-itself = { $keyword } をそれ自身の下には入れられません。
keywords-renaming = { $from } を { $to } に名前変更中…
keywords-merging = { $from } を { $to } に統合中…
keywords-renamed = { $from } を { $to } に変更しました（写真 { $count ->
   *[other] { $count } 枚
}）
keywords-rename-row = キーワード { $from } → { $to }
keywords-rename-finding = 写真を探しています…
keywords-rename-row-detail = 写真 { $done } / { $total } 枚

develop-split-moved = 境界線 { PERCENT($percent) }

develop-rules-placeholder-edit = 変更または削除するルールを選んでください
develop-rule-edit-placeholder = { $rule }
develop-rule-maker = メーカー：{ $value }
develop-rule-model = モデル：{ $value }
develop-rule-lens = レンズ：{ $value }
develop-rule-iso = ISO：{ $value }
develop-rule-type = ファイルの種類：{ $value }
develop-rule-style = スタイル：{ $value }
develop-rule-any = すべて
develop-rule-type-any = すべてのファイル
develop-rule-delete = ルールを削除…
develop-rule-field-title = 「{ $rule }」を変更
develop-rule-field-maker = メーカー（名前の一部で可。空欄：すべて）
develop-rule-field-model = モデル（名前の一部で可。空欄：すべて）
develop-rule-field-lens = レンズ（名前の一部で可。空欄：すべて）
develop-rule-field-iso = ISO。例：100-800、1600-、-400（空欄：すべて）
develop-rule-field-ok = 設定
develop-rule-iso-bad = 100-800 のような ISO の範囲を入力してください。
develop-rule-style-placeholder = ルールのスタイルを選んでください
