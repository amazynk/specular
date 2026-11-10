# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = メタデータを編集
palette-edit-metadata-help = 選択した写真のタイトル、キャプション、作成者、権利、場所を編集します。
metaedit-menu = メタデータを編集…
metaedit-title = メタデータ
metaedit-close = メタデータを閉じる
metaedit-photos = { $count ->
   *[other] 写真 { $count } 枚
}
metaedit-none-selected = メタデータを編集する写真を選択してください。
metaedit-reading = メタデータを読み込み中…
metaedit-mixed = 混在: 入力するとすべてに設定
metaedit-clearing = すべての写真で消去されます。Esc で元の値を残せます。
metaedit-keywords-others = 一部の写真にはほかにもあり
metaedit-saved = { $field } を保存しました。
metaedit-cleared = { $field } を消去しました。
metaedit-write-failed = { $count ->
   *[other] 写真 { $count } 枚のメタデータを保存できませんでした: { $error }
}
metaedit-writes-sidecars = XMP サイドカーに保存されます。
metaedit-writes-files = JPEG、TIFF、DNG ファイルに保存されます。その他のファイルには XMP サイドカーが作成されます。

metaedit-group-templates = テンプレート
metaedit-group-description = 説明
metaedit-group-creator = 作成者
metaedit-group-rights = 権利
metaedit-group-shown = 写っている場所
metaedit-group-created = 撮影場所
metaedit-group-workflow = ワークフロー
metaedit-templates-none = テンプレートはまだありません。テンプレートを使うと、よく使うフィールドをまとめて入力できます。

metaedit-field-title = タイトル
metaedit-field-headline = 見出し
metaedit-field-description = キャプション
metaedit-field-keywords = キーワード（; 区切り）
metaedit-field-persons-shown = 写っている人物（; 区切り）
metaedit-field-event = イベント
metaedit-field-creator = 作成者（; 区切り）
metaedit-field-creator-job-title = 作成者の役職
metaedit-field-contact-address = 住所
metaedit-field-contact-city = 市区町村
metaedit-field-contact-region = 都道府県/州
metaedit-field-contact-postal-code = 郵便番号
metaedit-field-contact-country = 国
metaedit-field-contact-phone = 電話
metaedit-field-contact-email = メール
metaedit-field-contact-url = Web サイト
metaedit-field-copyright = 著作権情報
metaedit-field-credit = クレジット
metaedit-field-source = ソース
metaedit-field-usage-terms = 使用条件
metaedit-field-shown-sublocation = 地区
metaedit-field-shown-city = 市区町村
metaedit-field-shown-state = 都道府県/州
metaedit-field-shown-country = 国
metaedit-field-shown-country-code = 国コード
metaedit-field-created-sublocation = 地区
metaedit-field-created-city = 市区町村
metaedit-field-created-state = 都道府県/州
metaedit-field-created-country = 国
metaedit-field-created-country-code = 国コード
metaedit-field-instructions = 指示
metaedit-field-job-id = ジョブ ID

metaedit-template-new = 新規テンプレート
metaedit-template-edit = 編集
metaedit-template-remove = 削除
metaedit-template-done = 完了
metaedit-template-name = テンプレート名
metaedit-template-default-name = テンプレート { $n }
metaedit-template-title = テンプレート
metaedit-template-hint = 空のフィールドは変更されません。
metaedit-template-replace-keywords = 写真のキーワードを置き換える
metaedit-template-applied = { $count ->
   *[other] 「{ $name }」を写真 { $count } 枚に適用しました。
}
metaedit-template-remove-ask = テンプレート「{ $name }」を削除しますか？
metaedit-template-remove-keep = 写真のメタデータはそのまま残ります。
metaedit-template-remove-yes = 削除
metaedit-template-removed = テンプレート「{ $name }」を削除しました。

app-info-edit-metadata = メタデータを編集…

settings-gallery-metadata-into-files = メタデータを画像ファイルに書き込む
settings-gallery-metadata-into-files-help = メタデータエディターは JPEG、TIFF、DNG ファイルにも書き込みます。ファイルのほかの部分は変更しません。オフ: XMP サイドカーのみ。
