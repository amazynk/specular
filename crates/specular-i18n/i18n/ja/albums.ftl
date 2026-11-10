# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = アルバム
albums-new = 新規アルバム…
albums-new-smart = 新規スマートアルバム…
albums-save-search = 検索をスマートアルバムとして保存…
albums-smart = スマートアルバム
# The album the photos go to when added without choosing.
albums-target = 追加先
albums-group-count = { $count ->
   *[other] { $count } 個のアルバム
}
albums-need-library = アルバムはライブラリにあります。先にこのフォルダーをライブラリに追加してください。

## Screen readers: the buttons of album and group rows
albums-a11y-rename = { $album } の名前を変更
albums-a11y-edit = { $album } を編集
albums-a11y-delete = アルバム { $album } を削除
albums-a11y-rename-group = グループ { $group } の名前を変更
albums-a11y-delete-group = グループ { $group } を削除

## Sheets
albums-name = 名前
albums-unnamed = 名前なし
albums-new-title = 新規アルバム
albums-new-smart-title = 新規スマートアルバム
albums-edit-smart-title = スマートアルバムを編集
albums-rename-title = アルバム名を変更
albums-rename-group-title = グループ名を変更
albums-create = 作成
albums-rename = 名前を変更
albums-save = 保存
albums-groups-note = / を使うとグループに入れられます: クライアント X / セレクト。
albums-name-empty = 名前を入力してください。
albums-name-taken = { $album } はすでにあります。
albums-name-clash = { $album } はアルバムなので、グループにはできません。

## Smart album rules
albums-rules = ルール
albums-rules-note = ルールは上から順に適用されます。空のルールは無視されます。
albums-rules-none = ルールを 1 つ以上追加してください。
albums-rule-label = ルール { $number }
albums-rule-empty = 空のルール
albums-rule-add = ルールを追加
albums-rule-combine = 上のルールと組み合わせる
albums-rule-first = 次に一致する写真
albums-rule-and = かつ
albums-rule-or = または
albums-rule-except = ただし次を除く

## Adding and removing photos
albums-add-menu = アルバムに追加…
albums-remove-menu = アルバムから外す
albums-add-title = { $count ->
   *[other] { $count } 枚の写真をアルバムに追加
}
albums-add = 追加
albums-add-new = 新規アルバム
albums-add-list = アルバム
albums-add-none = アルバムはまだありません。名前を入力すると作成できます。
albums-add-note = または新しいアルバムの名前を入力します。
albums-added = { $count ->
   *[other] { $count } 枚の写真を { $album } に追加しました。
}
albums-added-missing = { $count ->
   *[other] { $count } 枚はライブラリにないため、追加されませんでした。
}
albums-created = { $album } を作成しました。
albums-removed = { $count ->
   *[other] { $count } 枚の写真を { $album } から外しました。
}
albums-remove = 外す
albums-remove-confirm = { $count ->
   *[other] { $count } 枚の写真を { $album } から外しますか？
}
albums-remove-confirm-detail = ファイルはディスクとライブラリに残ります。
albums-delete = 削除
albums-delete-confirm = アルバム { $album } を削除しますか？
albums-delete-group-confirm = { $count ->
   *[other] グループ { $group } とその中の { $count } 個のアルバムを削除しますか？
}
albums-delete-confirm-detail = 写真はディスクとライブラリに残ります。
albums-deleted = { $count ->
    [1] アルバムを削除しました。
   *[other] { $count } 個のアルバムを削除しました。
}
albums-failed = アルバムを変更できませんでした: { $reason }

## Date timeline
albums-timeline = タイムライン
albums-timeline-description = 期間を選ぶと、その期間の写真だけを表示します。Shift+クリックで連続して選べます。
albums-timeline-picked = { $period } を表示中
albums-timeline-cleared = すべての日付を表示中
