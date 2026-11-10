# machine-drafted, needs native review
# Specular — バックグラウンド処理：バックグラウンドで何かが実行中に表示されるインジケーターと、それが開くパネル。日本語。

## パネル

work-title = バックグラウンド処理
work-show = バックグラウンド処理を表示
work-empty = バックグラウンドで実行中の処理はありません。

## 処理

work-kind-library = 写真ライブラリ
work-kind-ai-search = AI 検索
work-kind-faces = 人物
work-kind-text = 写真内のテキスト
work-kind-culling = 選別スコア
work-kind-download = AI モデルのダウンロード
work-kind-similar = 類似ショット

## ステップ

work-step-finding = 写真を検出中
work-step-reading = 写真の詳細を読み取り中
work-step-signatures = 写真を比較中
work-step-grouping = 類似ショットをグループ化中
work-step-text-index = 検索インデックスを作成中
work-step-embedding = AI 検索用に写真をインデックス登録中
work-step-switching = 新しいモデルサイズ用に再インデックス中
work-step-faces = 顔を検出中
work-step-ocr = 写真内のテキストを読み取り中
work-step-scoring = シャープネスと露出を評価中
work-step-downloading = ダウンロード中
work-step-focus = シャープネスを確認中
work-step-item = { $step } · { $item }

## 進行状況

work-count-photos = { $done } / { $total } { $count ->
   *[other] 枚
}
work-count = { $done } / { $total }
work-count-left = { $count } · 残り { $left }
work-so-far-photos = これまでに { $done } { $count ->
   *[other] 枚
}
work-so-far = これまでに { $done }

## 残り時間

work-eta-estimating = 推定中…
work-eta-seconds = 残り約 { $n ->
   *[other] { $n } 秒
}
work-eta-minutes = 残り約 { $n ->
   *[other] { $n } 分
}
work-eta-hours = 残り約 { $h ->
   *[other] { $h } 時間
}
work-eta-hours-minutes = 残り約 { $h ->
   *[other] { $h } 時間
} { $m ->
   *[other] { $m } 分
}

## 処理が待機している理由

work-paused-browsing = 閲覧中は一時停止
work-paused-battery = バッテリー駆動中は一時停止
work-paused-low-power = 省電力モード中は一時停止
work-paused-heat = コンピュータが高温のため一時停止
work-paused-memory = メモリ不足のため一時停止
work-paused-user = 一時停止中

## 次の処理

work-then = 次に：{ $steps }

## ボタン

work-pause = 一時停止
work-resume = 再開
work-settings = 設定…

## インジケーター

work-chip = { $job } · { $done } / { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = バックグラウンド処理：{ $status }

## 読み上げ

work-started = { $job }をバックグラウンドで開始しました
work-finished = { $job }が完了しました
