# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = ローカルコピーの上限
settings-library-local-copies-limit-help = Specular のデータフォルダでローカルコピーが使える最大の容量です。

local-keep-menu = ローカルコピーを保持…
local-remove-menu = ローカルコピーを削除…
local-keep-location-menu = この場所のローカルコピーを保持…
local-remove-location-menu = この場所のローカルコピーを削除…
local-keep-album-menu = このアルバムのローカルコピーを保持…
local-remove-album-menu = このアルバムのローカルコピーを削除…
local-remove-held-elsewhere = これらのコピーは保持中の場所またはアルバムに属しています。そちらで削除してください。

local-keep-ask = { $item } のローカルコピーを保持しますか？
local-keep-detail = { $count ->
   *[other] 写真 { $count } 枚。
}
    オリジナル：ファイルをそのままコピーします。
    プロキシ：RAW は 2,560 ピクセルで保存し、現像には十分です。その他のファイルはそのままコピーします。
    コピーは Specular のデータフォルダに置かれます。オリジナルは変更されません。
local-keep-originals = オリジナルを保持
local-keep-proxies = プロキシを保持
local-keeping = { $item } をコピー中…
local-kept = { $item }：{ $count ->
   *[other] ローカルコピー { $count } 件
}、{ $size }。
local-kept-full = { $count ->
   *[other] { $count } 枚の写真がサイズの上限に収まりませんでした。
}
local-kept-unreachable = { $count ->
   *[other] { $count } 枚の写真を読み込めませんでした。
}

local-remove-ask = { $item } のローカルコピーを削除しますか？
local-remove-detail = Specular のデータフォルダにあるコピーを削除します。オリジナルには触れません。待機中の変更は残ります。
local-remove-ok = ローカルコピーを削除
local-removed = { $item } のローカルコピーを削除し、{ $size } を空けました。

local-banner-offline = { $location } はオフラインです。ローカルコピーで作業しています。
local-banner-offline-many = { $count } か所の場所がオフラインです。ローカルコピーで作業しています。
local-banner-waiting = { $count ->
   *[other] { $count } 件の変更がオリジナルを待っています。
}
local-banner-conflicts = { $count ->
   *[other] オフラインの間に { $count } 件のファイルが変更されました。
}
local-review = 確認…
local-sidebar-originals = ローカルコピー
local-sidebar-proxies = ローカルプロキシ

local-synced = { $count ->
   *[other] { $count } 件の変更をオリジナルに書き込みました。
}
local-sync-failed = { $count ->
   *[other] { $count } 件の変更を書き込めませんでした。待機したままです。
}

local-conflict-title = オフラインの間に { $file } が変更されました。どちらの{ $what }を残しますか？
local-conflict-yours = あなたの変更：{ $text }
local-conflict-theirs = ディスク上：{ $text }
local-conflict-later = キャンセルすると後で決められます。何も上書きされません。
local-keep-mine = あなたの変更を残す
local-keep-theirs = ファイルの内容を残す
local-what-marks = レーティング
local-what-keywords = キーワード
local-what-fields = IPTC フィールド
local-what-edit = 編集
local-stars = { $count ->
    [0] 星なし
   *[other] 星 { $count } つ
}
local-label = { $label ->
    [1] 赤
    [2] 黄
    [3] 緑
   *[4] 青
}
local-pick = 採用
local-reject = 不採用
local-no-keywords = キーワードなし
local-changed-elsewhere = 別の場所で変更
local-photo-changed = 写真そのものが変更されました
local-edit-here = ここで編集、{ $when }
local-edit-elsewhere = 別の場所で編集、{ $when }
local-edit-none = 編集なし

local-export-ask = { $count ->
   *[other] { $count } 枚の写真がオフラインです。プロキシから書き出しますか？
}
local-export-detail = プロキシは長辺 2,560 ピクセルです。フルサイズにはオリジナルが必要です。
local-export-proxy = プロキシから書き出す
local-export-wait = オリジナルを待つ
local-export-waiting = オリジナルが戻ると書き出しが始まります。

local-panel-title = ローカルコピー
local-panel-used = { $limit } 中 { $used } を使用中。
local-panel-originals = { $count ->
   *[other] オリジナル { $count } 枚
}
local-panel-proxies = { $count ->
   *[other] 写真 { $count } 枚、RAW はプロキシ
}
local-panel-remove = 削除…
local-panel-footer = 保存先：{ $folder }。オリジナルは変更されません。
local-panel-keywords = オフライン ローカルコピー プロキシ 容量
