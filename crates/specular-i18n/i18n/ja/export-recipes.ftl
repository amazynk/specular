# Specular — Export recipes: saved ways to export, several run at once. machine-drafted, needs native review
# Every output is a new file (or replaces one only after asking); originals are never changed.

palette-export-recipes = レシピでエクスポート…
palette-export-recipes-help = 選択した画像を、保存した 1 つ以上のレシピでエクスポートします。
palette-develop-export-recipes = レシピでエクスポート…
palette-develop-export-recipes-help = 編集した写真（1 枚または複数）を保存したレシピでエクスポートします。

export-recipes-title = レシピでエクスポート
export-recipes-new = 新規
export-recipes-edit = 編集…
export-recipes-duplicate = 複製
export-recipes-delete = 削除…
export-recipes-start = エクスポート
export-recipes-save = 保存
export-recipes-open-folder = フォルダーを開く
export-recipes-choose-image = 画像を選択…
export-recipes-no-images = エクスポートする画像がありません。
export-recipes-none-checked = レシピを 1 つ以上選んでください。
export-recipes-checked = { $name }: オン
export-recipes-unchecked = { $name }: オフ

## The starters.
export-recipes-starter-web = Web、2048 px
export-recipes-starter-print = 印刷、フルサイズ
export-recipes-starter-email = メール、小
export-recipes-unnamed = 名前のないレシピ
export-recipes-new-name = 新しいレシピ
# { $name } is the recipe copied.
export-recipes-copy-name = { $name }（コピー）

## The list's one-line summary.
export-recipes-summary-mark = 透かし
export-recipes-size-full = フルサイズ
export-recipes-size-long = 長辺 { $px } px
export-recipes-size-short = 短辺 { $px } px
export-recipes-size-mp = { $mp } MP
export-recipes-size-exact = { $width } × { $height } px
export-recipes-size-percent = { PERCENT($percent) }
export-recipes-folder-same = 元のファイルと同じ場所
export-recipes-folder-sub = フォルダー「{ $name }」
export-recipes-folder-ask = 毎回確認

## The editor.
export-recipes-new-title = 新しいレシピ
export-recipes-edit-title = レシピを編集
export-recipes-name = レシピ名
export-recipes-text = カスタムテキスト
export-recipes-pattern = ファイル名
export-recipes-mark-text-field = 透かしのテキスト
export-recipes-subfolder = フォルダー名
export-recipes-format = 形式
export-recipes-quality = 品質
export-recipes-lossless = ロスレス
export-recipes-bits = ビット深度
export-recipes-bits-8 = 8 ビット
export-recipes-bits-16 = 16 ビット
export-recipes-space = 色空間
export-recipes-space-srgb = sRGB
export-recipes-space-p3 = Display P3
export-recipes-space-adobe = Adobe RGB (1998)
export-recipes-space-prophoto = ProPhoto RGB
export-recipes-profile = プロファイル
export-recipes-profile-embed = 埋め込む
export-recipes-profile-none = 含めない
export-recipes-folder = フォルダー
export-recipes-folder-subfolder = サブフォルダー
export-recipes-collision = 名前が使用済みのとき
export-recipes-collision-unique = 番号を付ける
export-recipes-collision-overwrite = 置き換える（確認あり）
export-recipes-collision-skip = スキップ
export-recipes-after = 完了後
export-recipes-after-nothing = 何もしない
export-recipes-after-open = フォルダーを開く
export-recipes-resize = サイズ
export-recipes-resize-long = 長辺
export-recipes-resize-short = 短辺
export-recipes-resize-mp = メガピクセル
export-recipes-resize-exact = 指定サイズ
export-recipes-resize-percent = パーセント
export-recipes-edge = 辺
export-recipes-width = 幅
export-recipes-height = 高さ
export-recipes-megapixels = メガピクセル
export-recipes-percent = パーセント
export-recipes-px = { $px } px
export-recipes-mp = { $mp } MP
export-recipes-pct = { PERCENT($percent) }
export-recipes-enlarge = 拡大する
export-recipes-sharpen = シャープの対象
export-recipes-sharpen-off = オフ
export-recipes-sharpen-screen = 画面
export-recipes-sharpen-matte = マット紙
export-recipes-sharpen-glossy = 光沢紙
export-recipes-amount = 適用量
export-recipes-amount-low = 弱
export-recipes-amount-standard = 標準
export-recipes-amount-high = 強
export-recipes-metadata = メタデータ
export-recipes-meta-all = すべて
export-recipes-meta-copyright = 著作権のみ
export-recipes-meta-contact = 著作権と連絡先
export-recipes-meta-none = なし
export-recipes-location = 位置情報
export-recipes-people = 写っている人物
export-recipes-keep = 保持
export-recipes-remove = 削除
export-recipes-mark = 透かし
export-recipes-mark-none = なし
export-recipes-mark-text = テキスト
export-recipes-mark-image = 画像（選択）
export-recipes-position = 位置
export-recipes-pos-top-left = 左上
export-recipes-pos-top = 上
export-recipes-pos-top-right = 右上
export-recipes-pos-left = 左
export-recipes-pos-centre = 中央
export-recipes-pos-right = 右
export-recipes-pos-bottom-left = 左下
export-recipes-pos-bottom = 下
export-recipes-pos-bottom-right = 右下
export-recipes-opacity = 不透明度
export-recipes-scale = 幅
export-recipes-yes = はい
export-recipes-no = いいえ
export-recipes-error-name = レシピに名前を付けてください。
export-recipes-error-mark-text = 透かしのテキストを入力してください。
export-recipes-error-mark-image = 透かしの画像を選んでください。
export-recipes-error-subfolder = フォルダー名を入力してください。
export-recipes-saved = レシピ「{ $name }」を保存しました。

## Delete.
export-recipes-delete-title = レシピ「{ $name }」を削除しますか？
export-recipes-delete-detail = このレシピでエクスポートしたファイルはディスクに残ります。
export-recipes-delete-confirm = 削除
export-recipes-delete-keep = 残す
export-recipes-deleted = レシピ「{ $name }」を削除しました。

## The run.
# { $name } is the recipe whose folder is asked for.
export-recipes-ask-title = 「{ $name }」のフォルダー
export-recipes-asking = 確認するレシピごとにフォルダーを選んでください。
export-recipes-no-folder = フォルダーが選ばれていません。何もエクスポートされませんでした。
export-recipes-planning = ファイル名を決めています…
export-recipes-plan-pattern = レシピ「{ $name }」: { $reason }
export-recipes-replace-title = { $count ->
   *[other] { $count } 個のファイルを置き換えますか？
}
export-recipes-replace-detail = 元のファイルが置き換えられることはありません。
export-recipes-replace-confirm = 置き換える
export-recipes-replace-kept = 何もエクスポートされませんでした。
export-recipes-running = エクスポート中…
export-recipes-progress = { $done } / { $total } ファイル
export-recipes-done = { $count ->
   *[other] { $count } 個のファイルをエクスポートしました。
}
export-recipes-stopped = { $count ->
   *[other] 停止しました。{ $count } 個のファイルをエクスポートしました。
}
export-recipes-done-skipped = { $count ->
   *[other] { $count } 個をスキップ: 名前が使用済みでした。
}
export-recipes-done-failed = { $count ->
   *[other] { $count } 個が失敗:
}
# One failure: the source file, the recipe and why.
export-recipes-failure = { $file } ({ $recipe }): { $reason }
