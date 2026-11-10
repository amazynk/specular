# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. 日本語. machine-drafted, needs native review

palette-tether = テザー撮影…
palette-tether-capture = 撮影する（テザー）

tether-title = テザー撮影
tether-close = テザー撮影を閉じる
tether-default-folder = テザー撮影

## Cameras
tether-none = カメラが見つかりません。
tether-none-hint = USB ケーブルでカメラを接続し、電源を入れてください。USB モードがある場合は PTP または PC 接続を選んでください。
tether-look-again = 再検索
tether-connect = 接続
tether-next-camera = 次のカメラ
tether-connecting = 接続中…
tether-connected = { $camera } を接続しました
tether-disconnect = 切断
tether-try-again = 再試行
tether-gone = カメラが切断されました。
tether-not-connected = カメラが接続されていません。先にテザー撮影を開いてください。

## Why a camera can't be opened
tether-permission = このユーザーにはカメラの USB デバイス { $node } を開く権限がありません。
tether-permission-rule = udev ルールで許可できます。管理者として次の行を { $file } に書き込んでください：
tether-permission-reload = その後「sudo udevadm control --reload」を実行し、カメラを接続し直してください。
tether-copy-rule = ルールをコピー
tether-busy-other = 別のプログラムがカメラを使用しています。
tether-busy-named = カメラは { $programs } が使用中です。
tether-busy-hint = そのプログラムを閉じるか、ファイルマネージャーでカメラをアンマウントしてから再試行してください。
tether-driver = このカメラのシステムドライバーは、ほかのプログラムからの使用を許可していません。

## The camera
tether-battery = バッテリー { PERCENT($percent) }
tether-card = { $card }：空き { $free }
tether-card-shots = { $card }：空き { $free }、残り { $shots } 枚
tether-no-card = メモリーカードがありません。
tether-no-settings = カメラが露出設定を報告しません。
tether-no-remote = このカメラはここからシャッターを切れません。カメラのシャッターボタンで撮った写真はダウンロードされます。
tether-capture = 撮影
tether-step-down = { $setting }：下げる（{ $value }）
tether-step-up = { $setting }：上げる（{ $value }）

## Exposure settings
tether-prop-mode = 撮影モード
tether-prop-shutter = シャッター速度
tether-prop-aperture = 絞り
tether-prop-iso = ISO 感度
tether-prop-bias = 露出補正
tether-prop-white-balance = ホワイトバランス
tether-auto = オート
tether-bulb = バルブ
tether-camera-value = カメラの設定 { $value }
tether-wb-manual = マニュアル
tether-wb-one-push = ワンプッシュ
tether-wb-daylight = 太陽光
tether-wb-fluorescent = 蛍光灯
tether-wb-tungsten = 白熱電球
tether-wb-flash = フラッシュ
tether-mode-manual = マニュアル
tether-mode-program = プログラム
tether-mode-aperture = 絞り優先
tether-mode-shutter = シャッター優先
tether-mode-creative = クリエイティブ
tether-mode-action = スポーツ
tether-mode-portrait = ポートレート

## Where shots go
tether-folder = フォルダー
tether-names = ファイル名
tether-names-camera = カメラの名前のまま
tether-arrival = Develop のスタイル
tether-choose-folder = フォルダー…
tether-choose-style = スタイル…
tether-downloading = ダウンロード中… { PERCENT($percent) }
tether-saved = { $name } を保存しました
tether-saved-count = { $count ->
   *[other] { $count } 枚を保存しました
}

## Failures
tether-busy = カメラがビジー状態です。再試行してください。
tether-card-full = メモリーカードがいっぱいです。
tether-set-refused = カメラがその設定を受け付けませんでした。モードダイヤルを確認してください。
tether-disk = 写真を保存できませんでした：{ $error }
tether-error = カメラのエラー：{ $error }

## Settings › Cull mode
settings-cull-tether-folder = テザー撮影のフォルダー
settings-cull-tether-folder-help = ケーブルで接続したカメラの写真を保存する場所。
settings-cull-tether-folder-unset = ピクチャ › テザー撮影
settings-cull-tether-rename = テザー撮影のファイル名
settings-cull-tether-rename-help = 読み込みの名前変更と同じトークンで付ける新しい名前。空欄ならカメラの名前のままです。
