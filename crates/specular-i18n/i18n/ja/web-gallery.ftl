# Specular — クライアントギャラリー：写真の静的 HTML フォルダーと、クライアントの選択の読み込み。

# machine-drafted, needs native review

palette-client-gallery = クライアントギャラリー…
palette-client-gallery-help = 写真をクライアント向けのウェブページのフォルダーとして書き出します。クライアントは選んだ写真を送り返せます。
palette-import-client-picks = クライアントの選択を読み込む…
palette-import-client-picks-help = クライアントがギャラリーで選んだ写真に印を付けます。

webgallery-heading = クライアントギャラリー
webgallery-subtitle = { $count ->
   *[other] 写真 { $count } 枚
}
webgallery-title-label = タイトル
webgallery-title-default = 校正用
webgallery-watermark-label = 透かしの文字（空欄：なし）
webgallery-source = 写真
webgallery-source-current = この写真
webgallery-source-selection = 選択範囲
webgallery-source-album = アルバム「{ $name }」
webgallery-source-search = 検索結果
webgallery-source-folder = このフォルダー
webgallery-source-picks = 採用
webgallery-source-value = { $what }（{ $count }）
webgallery-theme = テーマ
webgallery-theme-light = ライト
webgallery-theme-dark = ダーク
webgallery-theme-minimal = ミニマル
webgallery-size = 長辺
webgallery-size-value = { $px } px
webgallery-format = 形式
webgallery-captions = 各写真の下
webgallery-captions-off = なし
webgallery-captions-name = ファイル名
webgallery-captions-title = タイトル
webgallery-captions-all = ファイル名、タイトル、キャプション
webgallery-exif = カメラと露出を表示
webgallery-location = 写真に位置情報（GPS）を残す
webgallery-note-private = 静的なギャラリーはパスワードで保護できません。
webgallery-note-share = フォルダーやそのアドレスを持つ人は誰でもすべての写真を見られます。リンクはクライアントとだけ共有するか、ウェブホストのアクセス制御を使ってください。
webgallery-note-picks = クライアントはハートで選びます。選択は、クライアントがリストを送るまでブラウザーに残ります。
webgallery-write = ギャラリーを書き出す…
webgallery-busy = 書き出し中 { $done } / { $total }…
webgallery-stop = 中止
webgallery-done = { $count ->
   *[other] ギャラリーを書き出しました：{ $folder } に写真 { $count } 枚
}
webgallery-done-failed = { $failed ->
   *[other] 写真 { $failed } 枚を書き出せませんでした。
}
webgallery-stopped = ギャラリーを中止しました。何も残っていません。
webgallery-failed = ギャラリーを書き出せませんでした：{ $reason }
webgallery-nothing = 写真を 1 枚も書き出せませんでした。

webgallery-page-count = { $count ->
   *[other] 写真 { $count } 枚
}
webgallery-page-hint = 気に入った写真のハートをタップしてください。選択は送信するまでこのブラウザーに残ります。
webgallery-page-picked = 選択済み：{"{"}n{"}"}
webgallery-page-pick = 選ぶ
webgallery-page-copy = 選択をコピー
webgallery-page-copied = コピーしました。リストをメッセージに貼り付けてください。
webgallery-page-copy-by-hand = 選択されたリストをコピーして、メッセージに貼り付けてください。
webgallery-page-download-txt = picks.txt をダウンロード
webgallery-page-download-json = picks.json をダウンロード
webgallery-page-clear = 選択を消去
webgallery-page-clear-ask = このブラウザーの選択をすべて消去しますか？
webgallery-page-no-storage = このブラウザーは選択を保存しません。ページを閉じる前に送ってください。
webgallery-page-noscript = 選ぶには JavaScript が必要です。写真は開けます。
webgallery-page-previous = 前へ
webgallery-page-next = 次へ
webgallery-page-close = 閉じる
webgallery-page-position = {"{"}i{"}"} / {"{"}n{"}"}
webgallery-page-viewer = 写真ビューアー
webgallery-page-picks = あなたの選択

clientpicks-heading = クライアントの選択を読み込む
clientpicks-subtitle = { $count ->
   *[other] 表示中の写真 { $count } 枚と照合
}
clientpicks-paste-label = 名前（クライアントのリストを貼り付け）
clientpicks-choose = 選択ファイルを選ぶ…
clientpicks-from-file = { $name } から
clientpicks-apply-as = 印の付け方
clientpicks-as-pick = 採用として
clientpicks-as-stars = { $stars ->
   *[other] 星 { $stars } つ
}
clientpicks-as-label = ラベル { $label }
clientpicks-empty = クライアントが送ったリストを貼り付けるか、picks.txt または picks.json を選んでください。
clientpicks-matched = { $total } 件の名前のうち { $matched } 件がここの写真と一致します。
clientpicks-missing = 見つかりません：{ $names }
clientpicks-ambiguous = 複数のフォルダーにあるため変更しません：{ $names }
clientpicks-more = { $names } ほか { $more } 件
clientpicks-apply = 適用
clientpicks-applied = { $count ->
   *[other] 写真 { $count } 枚に印を付けました。
}
clientpicks-unchanged = それらの写真にはすでに印が付いています。
clientpicks-read-failed = { $name } を読み込めません：{ $reason }
