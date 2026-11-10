# machine-drafted, needs native review
# Specular — Library (DESIGN §10).

palette-library = ライブラリ

## Top bar
library-title = ライブラリ
library-locations-button = 場所

## Locations sidebar
library-locations = 場所
library-all = すべての場所
library-add = 場所を追加…
library-remove = { $location } をライブラリから削除
library-offline = オフライン
library-location-pending = インデックス待ち { $count } 件
library-a11y-location = { $location }, { $count ->
   *[other] { $count } 枚の画像
}
library-a11y-location-offline = { $location }, オフライン, { $count ->
   *[other] { $count } 枚の画像
}

## Indexing progress
library-walking = 見つかった画像：{ $count } · { $location }
# machine-drafted, needs native review
library-walking-count = 見つかった画像：{ $count }
library-reading = インデックス作成中… { $done } / { $total }
library-held = 閲覧中はインデックス作成を一時停止します
library-up-to-date = 最新
# machine-drafted, needs native review
library-stopped = インデックス作成を停止しました
library-stopped-pending = インデックス作成を停止しました · 残り { $count } 件
library-resume = 再開
# machine-drafted, needs native review
# The indexer stopped by itself (a database error, a bug) or stopped moving; $reason says why.
library-halted = インデックス作成を停止しました：{ $reason }
library-reason-stalled = { $minutes ->
   *[other] { $minutes } 分間
}、何も進んでいません。ファイルまたはドライブが応答していない可能性があります。
library-retry = 再試行
library-retry-description = インデックス作成をやり直します。作成済みの分は検索できます。
library-save-diagnostics = 診断パッケージを保存…
library-stop-description = インデックス作成を停止します。作成済みの分は検索できます。再開または再スキャン (F5) で続行します。
library-resume-description = すべての場所をもう一度調べ、残りをインデックスに登録します。
library-opening = ライブラリを開いています…
library-searching = 検索中…

## Empty and error states
library-empty = まだ場所がありません。フォルダーを追加すると、その中とサブフォルダーの画像がここに 1 つのグリッドで表示されます。
library-empty-offline = これらの場所はオフラインです。戻ると画像が再び表示されます。
library-indexing = 画像を検索中…
# machine-drafted, needs native review
library-remove-confirm = 「{ $location }」をライブラリから削除しますか？
library-remove-confirm-detail = 写真はレーティング、ラベル、キーワードとともにディスクに残ります。ライブラリはそこでインデックスした内容を忘れます。フォルダーを再度追加すると、もう一度インデックスを作成します。
library-remove-confirm-ok = 削除
library-indexing-in = { $location } で画像を検索中… これまでに { $count } 件
# machine-drafted, needs native review
library-empty-stopped = これらの画像を読み込む前にインデックス作成が停止されました。再開するとそこから続行します。
library-unavailable = ライブラリを開けませんでした: { $reason }
library-read-only = このライブラリは新しい Specular で書き込まれました。検索はできますが、更新はできません。

## Screen readers
library-a11y-list = ライブラリ, { $count ->
   *[other] { $count } 枚の画像
}
library-a11y-added = { $location } をライブラリに追加しました
library-a11y-removed = { $location } をライブラリから削除しました

## Adding the home folder
# machine-drafted, needs native review
library-home-confirm = 「{ $folder }」の中身をすべて追加しますか？
library-home-confirm-detail = 写真だけでなく、プログラムのファイルやダウンロード、キャッシュも含まれます。インデックス作成に時間がかかり、ライブラリがアイコンなどの画像であふれます。
library-home-confirm-pictures = 「ピクチャ」フォルダーには、あなたの画像だけが入っています。
library-home-confirm-use-pictures = 「ピクチャ」を追加
library-home-confirm-keep = ホームフォルダーを追加

## Folder watches
# machine-drafted, needs native review
library-unwatched = 監視されていないフォルダーがあります
library-unwatched-detail = { $minutes ->
   *[other] 変更は { $minutes } min 以内に反映されます
}
library-unwatched-description = Specular のフォルダー監視の上限に達しました。上限を引き上げる方法のヘルプを開きます。

## Settings → Library
# machine-drafted, needs native review
settings-section-library = ライブラリ
settings-library-skip-folders = 除外するフォルダー名
settings-library-skip-folders-help = ライブラリは、この名前のフォルダーの中を調べません。カンマで区切ります。隠しフォルダーは常に除外されます。
settings-library-skip-marked = 除外するフォルダーの内容
settings-library-skip-marked-help = 次のいずれかを含むフォルダーの中も調べません。コードのチェックアウトは .git、Python 環境は pyvenv.cfg、キャッシュは CACHEDIR.TAG。場所そのもののフォルダーは除外されません。
settings-library-show-timeline = タイムライン
settings-library-show-timeline-help = ライブラリのグリッドの上に、年・月・日ごとの写真の数を表示します。
