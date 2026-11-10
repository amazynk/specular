# machine-drafted, needs native review
## Astronomy interop: DS9 regions, plate solving, SAMP, HiPS sky surveys.

palette-science-load-regions = DS9 リージョンを重ねる…
palette-science-load-regions-help = DS9 リージョンファイル (.reg) の図形を画像上に描きます。
palette-science-export-regions = リージョンを DS9 形式で保存…
palette-science-export-regions-help = 計測と重ねたリージョンを DS9 リージョンファイルとして保存します。
palette-science-edit-regions = リージョンを計測として編集
palette-science-edit-regions-help = 重ねたリージョンを、移動や変更ができる計測に変換します。
palette-science-clear-regions = 重ねたリージョンを削除
palette-science-clear-regions-help = 画像から DS9 リージョンを削除します。
palette-science-plate-solve = プレートソルブ
palette-science-plate-solve-help = ASTAP または astrometry.net で画像の天球座標を求めます。もう一度実行すると停止します。
palette-science-save-plate-solution = プレートソルブの結果を保存
palette-science-save-plate-solution-help = 求めた天球座標を画像の隣に .wcs ファイルとして保存します。
palette-science-sky-survey = スカイサーベイの背景
palette-science-sky-survey-help = 画像の周りに次のスカイサーベイを表示します (または表示しません)。
palette-science-samp-send-image = 画像を SAMP アプリケーションに送る
palette-science-samp-send-image-help = この画像を DS9、Aladin など SAMP ハブ上のアプリケーションで開きます。
palette-science-samp-send-position = 位置を SAMP アプリケーションに送る
palette-science-samp-send-position-help = ポインター位置の天球上の位置を他のアプリケーションに示します。

disabled-samp-off = 先に SAMP ハブに接続してください（設定 › 統合）。

settings-integration-samp = SAMP ハブに接続
settings-integration-samp-help = DS9、TOPCAT、Aladin などの天文アプリケーションと画像、テーブル、天球上の位置をやり取りします。このコンピューター上のハブだけを使います。
settings-integration-astap-path = ASTAP プログラム
settings-integration-astap-path-help = 自動で見つからない場合の ASTAP プレートソルバー。
settings-integration-solve-field-path = astrometry.net プログラム
settings-integration-solve-field-path-help = 自動で見つからない場合の astrometry.net の solve-field プログラム。

astro-regions-filter = DS9 リージョン
astro-regions-loaded = { $name }: リージョン { $count } 個。
astro-regions-refused = { $count } 行を読めませんでした。最初: { $line } 行目、{ $reason }。
astro-regions-approximate = panda { $count } 個を内側と外側の縁で描きました。
astro-regions-none-here = { $name }: この画像にかかるリージョンはありません。
astro-regions-failed = { $name } を読めませんでした: { $reason }
astro-regions-cleared = リージョンを削除しました。
astro-regions-nothing = 保存するリージョンや計測がありません。
astro-regions-saved = { $name } を保存しました。
astro-regions-skipped = 天球座標の範囲外にある図形 { $count } 個を除外しました。
astro-regions-save-failed = リージョンを保存できませんでした: { $reason }
astro-regions-edited = リージョン { $count } 個を計測にしました。
astro-regions-no-wcs = 天球上のリージョンには天球座標のある画像が必要です。

astro-solve-no-solver = プレートソルバーが見つかりません。ASTAP か astrometry.net をインストールするか、設定 › 統合 で選んでください。
astro-solve-running = { $solver } で解析中… { $seconds } 秒
astro-solve-stop = 停止
astro-solve-stopped = プレートソルブを停止しました。
astro-solve-done = 解決: { $ra }、{ $dec }、1 ピクセルあたり { $scale }。
astro-solve-failed = プレートソルブに失敗しました: { $reason }
astro-solve-not-local = このコンピューター上のフォルダーにある画像だけを解析できます。
astro-solve-none = この画像にはまだプレートソルブの結果がありません。
astro-solve-saved = { $name } を保存しました。画像自体は変更されていません。
astro-solve-sidecar = { $name } の天球座標。

astro-samp-connected = SAMP ハブに接続しました。
astro-samp-no-hub = SAMP: { $reason }。ハブが起動したら再試行します。
astro-samp-lost = SAMP ハブが停止しました。ハブが起動したら再試行します。
astro-samp-sent = { $count } 個のアプリケーションに送りました。
astro-samp-nobody = ハブ上にこれを受け取るアプリケーションはありません。
astro-samp-failed = SAMP: { $reason }
astro-samp-refused = SAMP: { $url } はこのコンピューター上にないため、開きません。
astro-samp-pointed = { $ra }、{ $dec } に移動しました。
astro-samp-off-image = { $ra }、{ $dec } はこの画像上にありません。
astro-samp-no-table = SAMP: そのテーブルはここに重ねられていません。
astro-samp-rows = カタログの天体 { $count } 個を強調表示しました。

astro-survey-off = スカイサーベイ: オフ。
astro-survey-on = スカイサーベイ: { $name }。{ $credit }
astro-survey-failed = { $name } を表示できません: { $reason }
astro-survey-credit = { $name }: { $credit }

net-what-sky-survey = { $host } のスカイサーベイのタイル
