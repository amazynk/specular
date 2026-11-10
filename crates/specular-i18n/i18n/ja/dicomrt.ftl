# DICOM 構造化レポート、波形、放射線治療の線量、眼科断層撮影。 machine-drafted, needs native review

## 構造化レポートのコンテンツツリー (レポートモードのタグブラウザー)
sr-tree-title = { $title } ({ $kind })
sr-col-item = 項目
sr-col-concept = 概念
sr-col-type = 種類
sr-col-relationship = 関係
sr-col-value = 値
sr-show-image = 画像を表示
sr-jump-missing = 参照先の画像はこのフォルダーにありません
sr-jump-frame = この座標系にはその点を含む画像がありません

## グラフ (線量体積ヒストグラム、心電図シート)
plot-caliper-time = { $ms } ms · { $rate } /分
plot-caliper-time-zero = 0 ms
plot-caliper-hint = グラフ上をドラッグして計測
plot-adjust-hint = + と − でゲイン、[ と ] で速度を変更

## 心電図シート
ecg-paper = { $speed } mm/s · { $gain } mm/mV · { $rate } Hz
ecg-groups-more = { $count ->
   *[other] 表示されていないチャンネルグループがあと { $count } 個あります
}

## MPR 上の放射線治療線量 (等線量曲線、カラーマップ、凡例、読み取り値)
dose-value = 線量 { $value } { $unit } ({ PERCENT($percent) })
dose-hidden = 線量は非表示 (Shift+U で表示)
dose-on-rx = 線量: 処方 { $dose } { $unit } · 最大 { $max } { $unit }
dose-on-max = 線量: 最大値に対する割合、{ $max } { $unit }
dose-colour = 線量を { $colormap } で表示、{ PERCENT($blend) }
dose-dvh-hint = { $count ->
   *[other] Shift+K: { $count } 個の構造の線量体積ヒストグラム
}
dose-level = { PERCENT($percent) } · { $dose } { $unit }
dose-mode-lines = 線量を等線量曲線で表示
dose-mode-colour = 線量をカラーマップで表示

## 線量体積ヒストグラム (グラフシート)
dvh-title = 線量体積ヒストグラム
dvh-col-roi = 構造
dvh-col-volume = 体積
dvh-axis-dose = 線量 ({ $unit })
dvh-axis-volume = 体積 (%)
dvh-prescription = 処方線量 { $dose } Gy · 累積
dvh-max = 最大線量 { $dose } { $unit } · 累積
dvh-none = 線量のヒストグラムを求める、この座標系のストラクチャセットがありません

## 眼科断層撮影 (B スキャンのスライダー、隅のローカライザー画像)
oct-bscan = B スキャン
oct-depth = 深さ { $depth } µm
