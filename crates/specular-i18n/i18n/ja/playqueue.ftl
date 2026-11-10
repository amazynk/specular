# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = キュー・リピート・シャッフル
# machine-drafted, needs native review
palette-next-track = 次の項目
# machine-drafted, needs native review
palette-next-track-help = キューの次の項目を、キューがなければフォルダー内の同じ種類の次のファイルを再生します。
# machine-drafted, needs native review
palette-prev-track = 前の項目
# machine-drafted, needs native review
palette-prev-track-help = 前の項目を再生します。3 秒を過ぎていれば今の項目を最初から再生します。
# machine-drafted, needs native review
palette-cycle-repeat = リピート
# machine-drafted, needs native review
palette-cycle-repeat-help = リピートをオフ・すべて・1 項目に切り替えます。
# machine-drafted, needs native review
palette-shuffle = シャッフル
# machine-drafted, needs native review
palette-shuffle-help = ランダムな順に、各項目を 1 巡に 1 回ずつ再生します。
# machine-drafted, needs native review
palette-play-queue = 再生キュー
# machine-drafted, needs native review
palette-play-queue-help = 次に再生されるものを表示します。並べ替え、削除、保存ができます。
# machine-drafted, needs native review
palette-queue-play-next = 次に再生
# machine-drafted, needs native review
palette-queue-play-next-help = ファイル (ギャラリーでは選択項目) を再生中の項目の次に再生します。
# machine-drafted, needs native review
palette-queue-add = キューに追加
# machine-drafted, needs native review
palette-queue-add-help = ファイル (ギャラリーでは選択項目) をキューの最後に追加します。
# machine-drafted, needs native review
palette-save-queue = キューをプレイリストとして保存…
# machine-drafted, needs native review
palette-save-queue-help = キューを M3U8 または XSPF のプレイリストとして保存します。
# machine-drafted, needs native review
transport-prev-track = 前の項目
# machine-drafted, needs native review
transport-next-track = 次の項目
# machine-drafted, needs native review
queue-title = 再生キュー
# machine-drafted, needs native review
queue-summary = { $count ->
   *[other] { $count } 項目
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
   *[other] { $count } 項目
} · { $time }
# machine-drafted, needs native review
queue-empty = キューは空です。
    「次に再生」または「キューに追加」でサウンドファイルやビデオを追加するか、プレイリストを開いてください。
# machine-drafted, needs native review
queue-empty-short = キューは空です。
# machine-drafted, needs native review
queue-playing = 再生中
# machine-drafted, needs native review
queue-repeat = リピート
# machine-drafted, needs native review
queue-repeat-off = リピート: オフ
# machine-drafted, needs native review
queue-repeat-all = リピート: すべて
# machine-drafted, needs native review
queue-repeat-one = リピート: 1 項目
# machine-drafted, needs native review
queue-shuffle = シャッフル
# machine-drafted, needs native review
queue-shuffle-on = シャッフル: オン
# machine-drafted, needs native review
queue-shuffle-off = シャッフル: オフ
# machine-drafted, needs native review
queue-save = プレイリストとして保存…
# machine-drafted, needs native review
queue-save-tip = キューを M3U8 または XSPF のプレイリストとして保存
# machine-drafted, needs native review
queue-clear = クリア…
# machine-drafted, needs native review
queue-clear-tip = キューをクリアします。ファイルはディスクに残ります。
# machine-drafted, needs native review
queue-remove = キューから削除 (ファイルはディスクに残ります)
# machine-drafted, needs native review
queue-network-item = ネットワーク項目、再生しません: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = 次の項目はありません。
# machine-drafted, needs native review
queue-at-start = 前の項目はありません。
# machine-drafted, needs native review
queue-missing = { $name } が見つかりません。スキップしました。
# machine-drafted, needs native review
queue-network-not-played = ネットワーク項目はまだ再生できません。
# machine-drafted, needs native review
queue-nothing-to-add = 追加するものがありません。キューに入るのはサウンドファイル、ビデオ、プレイリストだけです。
# machine-drafted, needs native review
queue-added = { $count ->
   *[other] { $count } 項目をキューに追加しました。
}
# machine-drafted, needs native review
queue-added-next = { $count ->
   *[other] { $count } 項目を次に再生します。
}
# machine-drafted, needs native review
queue-playlist-opened = { $name }: { $count ->
   *[other] キューに { $count } 項目。
}
# machine-drafted, needs native review
queue-playlist-nothing = { $name } にはここで再生できるものがありません。キューに内容を表示します。
# machine-drafted, needs native review
queue-playlist-failed = { $name } を開けません: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = ファイルを読み込めません。
# machine-drafted, needs native review
queue-playlist-damaged = Specular が読み込めるプレイリストではありません。
# machine-drafted, needs native review
queue-clear-ask = { $count ->
   *[other] キューをクリアしますか ({ $count } 項目)?
}
# machine-drafted, needs native review
queue-clear-detail = クリアされるのは一覧だけです。ファイルはディスクに残ります。
# machine-drafted, needs native review
queue-clear-ok = キューをクリア
# machine-drafted, needs native review
queue-cleared = キューをクリアしました。
# machine-drafted, needs native review
queue-save-title = キューをプレイリストとして保存
# machine-drafted, needs native review
queue-default-name = キュー
# machine-drafted, needs native review
queue-format-m3u8 = M3U8 プレイリスト
# machine-drafted, needs native review
queue-format-xspf = XSPF プレイリスト
# machine-drafted, needs native review
queue-saved = { $name } を保存しました。
# machine-drafted, needs native review
queue-save-failed = { $name } を保存できません: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = 次のサウンドファイルを再生
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = サウンドファイルが終わると、フォルダー内の次のサウンドファイルを再生します。
# machine-drafted, needs native review
settings-viewing-play-repeat = リピート
# machine-drafted, needs native review
settings-viewing-play-repeat-help = サウンドファイル、ビデオ、キューに適用されます。
# machine-drafted, needs native review
settings-viewing-play-repeat-off = オフ
# machine-drafted, needs native review
settings-viewing-play-repeat-all = すべて
# machine-drafted, needs native review
settings-viewing-play-repeat-one = 1 項目
# machine-drafted, needs native review
settings-viewing-play-shuffle = シャッフル
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = ランダムな順に、各項目を 1 巡に 1 回ずつ再生します。
