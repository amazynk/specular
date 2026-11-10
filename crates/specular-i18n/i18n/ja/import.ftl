# machine-drafted, needs native review
# Specular — カメラ、スマートフォン、メモリーカードからの読み込み。日本語。
# カードは読み取るだけです。ファイルは、確認済みのコピーの後、指示があった場合にのみ削除されます。

palette-import = 写真を読み込む…
disabled-no-import-source = メモリーカード、カメラ、スマートフォンが接続されていません

## 設定 → 読み込み
settings-section-import = 読み込み
settings-import-on-card = カードを挿入したとき
settings-import-on-card-help = 写真の入ったメモリーカード、カメラ、スマートフォンを接続したときの動作です。
settings-import-on-card-ask = 確認する
settings-import-on-card-open = 読み込みを開く
settings-import-on-card-nothing = 何もしない
settings-import-destination = 読み込み先
settings-import-destination-help = 読み込んだ写真とビデオを保存するフォルダです。未設定のときはピクチャフォルダに保存します。
settings-import-destination-unset = ピクチャフォルダ
settings-import-folder-pattern = フォルダ
settings-import-folder-pattern-help = その中に作るフォルダ：{"{"}year{"}"}、{"{"}month{"}"}、{"{"}day{"}"}、{"{"}camera{"}"}、{"{"}event{"}"}。/ で区切ります。
settings-import-rename-pattern = 名前を変更
settings-import-rename-pattern-help = 新しいファイル名。変換と同じ書式です：{"{"}name{"}"}、{"{"}n:03{"}"}、{"{"}date:%Y%m%d{"}"}、{"{"}camera{"}"}、{"{"}event{"}"}。空欄ではカメラの名前のままです。
settings-import-keep-pairs = RAW と JPEG をまとめる
settings-import-keep-pairs-help = RAW ファイルと同時に撮影された JPEG を、名前を変えても同じフォルダに同じ名前で保存します。
settings-import-low-res-videos = 低解像度のビデオコピーも読み込む
settings-import-low-res-videos-help = 一部のカメラは各ビデオの小さなプレビュー用コピー（.LRV）を記録します。オフのときはカードに残します。
settings-import-eject = 完了したら取り出す
settings-import-eject-help = すべてコピーし終えたらカードを取り出し、抜けるようにします。

## カード挿入時の案内
import-offer = { $name } から読み込みますか？
import-offer-button = 読み込む

## 読み込み画面
import-title = 読み込み
import-from = { $name } から
import-source-next = 次のソース
import-close = 閉じる
import-scanning = 新しい写真を探しています… { $done } / { $total }
import-scanning-start = 新しい写真を探しています…
import-none-new = { $name } の内容はすべて読み込み済みです。
import-empty = { $name } に写真やビデオはありません。
import-no-source = メモリーカード、カメラ、スマートフォンが接続されていません。
import-summary = { $count ->
   *[other] 新しい項目 { $count } 件
} · { $size }
import-already = { $count } 件は読み込み済み
import-unreadable = { $count ->
   *[other] { $count } 個のファイルを読み取れませんでした
}
import-selected = { $count } / { $total } 件を選択
import-select-all = すべて選択
import-select-none = 選択を解除
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = ビデオ

import-destination = 読み込み先
import-choose = 選択…
import-folder-pattern = フォルダ
import-event = イベント
import-event-hint = 省略可。{"{"}event{"}"} に使います
import-rename = 名前を変更
import-rename-hint = 空欄ではカメラの名前のままです
import-preview = 最初のファイル：{ $path }
import-pattern-error = { $error }
import-keep-pairs = RAW と JPEG をまとめる
import-eject = 完了したら取り出す
import-delete = 読み込み後にカードから削除
import-free = コピーするサイズ { $size } · 空き容量 { $free }
import-free-unknown = コピーするサイズ { $size }
import-no-space = { $folder } の空き容量が足りません。この読み込みには { $need } 必要ですが、空きは { $free } だけです。空き容量を増やすか、別のフォルダを選んでください。
import-start = { $count ->
   *[other] { $count } 件を読み込む
}
import-nothing-selected = 読み込む写真を選択してください

import-confirm-delete = { $count ->
   *[other] コピーと確認の後、{ $name } から { $count } 個のファイルを削除します。元に戻せません。
}
import-confirm-yes = 読み込んで削除
import-confirm-no = 戻る

import-progress = 読み込み中 { $done } / { $total } · { $size }
import-progress-eta = 読み込み中 { $done } / { $total } · { $size } · { $time }
import-paused = 一時停止中 · { $done } / { $total }
import-pause = 一時停止
import-resume = 再開
import-cancel = キャンセル
import-hide = 隠す
import-stopping = 停止しています…
import-running = 読み込みはすでに実行中です。

import-done = { $count ->
   *[other] { $count } 個のファイルを
} { $folder } に読み込みました
import-done-existed = { $count } 個はすでにありました
import-done-deleted = { $count } 個をカードから削除しました
import-done-failed = { $count ->
   *[other] { $count } 個のファイルを読み込めませんでした
}
import-done-cancelled = { $count } 個のファイルの後で読み込みをキャンセルしました。もう一度読み込むと、止まったところから続けます。
import-done-nothing = 新しく読み込んだものはありません。
import-ejected = { $name } を取り外せます。
import-eject-failed = { $name } を取り出せませんでした：{ $error }
import-failed-destination = フォルダ { $folder } は使用できません：{ $error }
import-ledger-failed = 読み込み済みファイルの一覧を読み取れませんでした（{ $error }）。読み込み済みのファイルが再び表示されることがあります。

## スクリーンリーダー
import-a11y-group = 読み込み
import-a11y-tile = { $name }、{ $date }
import-a11y-tile-selected = { $name }、{ $date }、選択済み
