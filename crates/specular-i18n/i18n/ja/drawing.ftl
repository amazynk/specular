# Specular — Drawings (DXF): layers, paper, lineweights, model space and layouts. 日本語.

keys-mode-drawing = 図面
cheatsheet-drawing = 図面
palette-drawing-layers = 画層パネルの表示/非表示
palette-drawing-all-layers = すべての画層を表示、または保存時の画層に戻す
palette-drawing-paper = 暗い用紙と明るい用紙を切り替え
palette-drawing-lineweights = 線の太さの表示/非表示
disabled-not-drawing = 図面 (DXF) のみ
# A drawing's page: model space (layouts are shown by their own name).
page-label-model = モデル
# machine-drafted, needs native review
cad-pen-name = ペン { $n }
cad-page-name = ページ { $n }
cad-layer-name = 画層 { $n }
cad-map-name = 地図
# machine-drafted, needs native review
# A board's views (its Gerber files together): every layer stacked, and its top and bottom
# in board colours.
cad-board-layers-name = レイヤー
cad-board-top-name = 表面
cad-board-bottom-name = 裏面
# machine-drafted, needs native review
# A plot's layers: its frame, a trace without a name (numbered from 1), and the contour
# lines of a 2-D spectrum above and below zero.
cad-chart-frame-name = 枠
cad-trace-name = トレース { $n }
cad-contours-positive-name = 正の等高線
cad-contours-negative-name = 負の等高線
# machine-drafted, needs native review
# The page of a survey file (LandXML) showing its surfaces and alignments in 3D.
cad-model-3d-name = 3D モデル
# The layers panel's title (and its name for screen readers).
drawing-layers-title = 画層 · { $total } 件中 { $shown } 件を表示
# Toasts.
drawing-layers-all-toast = すべての画層を表示
drawing-layers-saved-toast = 図面に保存された画層
drawing-paper-dark-toast = 暗い用紙
drawing-paper-light-toast = 明るい用紙
drawing-lineweights-on = 線の太さを表示
drawing-lineweights-off = 線の太さオフ: 細線

# machine-drafted, needs native review
# Markup on drawings and images (Alt+M): kept in a file next to the drawing, never in it.
palette-markup = マークアップ
palette-markup-help = 雲マーク、テキスト、矢印、スタンプ、計測、写真ピン。ファイルの隣に保存します。
palette-compare-sheets = 図面を比較
palette-compare-sheets-help = この図面の別の版を位置合わせして重ね、変更箇所を囲みます。
markup-title = マークアップ
markup-list = このページのマークアップ
markup-count = { $count ->
    [0] このページにマークアップはありません
   *[other] このページに { $count } 件
}
markup-empty = ツールを選んで図面上をクリックしてください。
markup-item = { $n }. { $kind }
markup-tool-select = 選択
markup-tool-cloud = 雲マーク
markup-tool-text = テキスト
markup-tool-arrow = 矢印
markup-tool-stamp = スタンプ
markup-tool-length = 長さ
markup-tool-area = 面積
markup-tool-pin = 写真ピン
markup-hint-select = クリックで選択、ドラッグで移動。Enter でテキストを編集します。
markup-hint-cloud = 四角形をドラッグするか、角をクリックして Enter を押します。
markup-hint-text = テキストを置く位置をクリックします。
markup-hint-arrow = ラベルから指す点までドラッグするか、両方をクリックします。
markup-hint-stamp = スタンプを置く位置をクリックします。
markup-hint-length = ドラッグするか、両端をクリックします。
markup-hint-area = 角をクリックし、Enter を押します。
markup-hint-pin = 点をクリックし、写真を選びます。
markup-kind-cloud = 雲マーク
markup-kind-text = テキスト
markup-kind-arrow = 矢印
markup-kind-stamp = スタンプ
markup-kind-length = 長さ
markup-kind-area = 面積
markup-kind-pin = 写真
markup-colour = 色: { $colour }
markup-colour-red = 赤
markup-colour-blue = 青
markup-colour-green = 緑
markup-colour-orange = オレンジ
markup-colour-magenta = マゼンタ
markup-colour-black = 黒
markup-width = 線: { $width }
markup-width-thin = 細
markup-width-medium = 中
markup-width-thick = 太
markup-stamp = スタンプ: { $words }
markup-stamp-approved = 承認
markup-stamp-rejected = 却下
markup-stamp-reviewed = 確認済
markup-stamp-custom = 任意の文字
markup-undo = 取り消す
markup-redo = やり直す
markup-delete = 削除
markup-open-photo = 写真を開く
markup-from-measurements = 計測を追加
markup-from-measurements-done = { $count ->
   *[other] { $count } 件の計測をマークアップとして追加しました
}
markup-export-pdf = PDF を書き出す…
markup-export-png = PNG を書き出す…
markup-done = 完了
markup-ask-text = テキスト (Enter で配置、Esc でキャンセル)
markup-ask-label = ラベル (省略可、Enter で配置)
markup-ask-stamp = スタンプの文字 (Enter で配置)
markup-ask-edit = 新しいテキスト (Enter で保存)
markup-added = { $kind } を追加しました
markup-moved = 移動しました
markup-pin-hint = ピンをもう一度クリックするか Enter を押すと写真を開きます。
markup-deleted = 削除しました
markup-undone = 取り消しました
markup-redone = やり直しました
markup-delete-ask = { $name } を削除しますか？
markup-delete-detail = マークアップファイルから削除されます。図面そのものは変更されません。
markup-delete-ok = 削除
markup-photo-missing = { $name } が見つかりません。
markup-load-failed = マークアップを読み込めません: { $reason }
markup-save-failed = マークアップを保存できません: { $reason }
markup-sidecar-too-large = マークアップファイルが大きすぎます
markup-pdf-unreadable = PDF を読み込めませんでした
markup-exported = { $name } を保存しました
markup-export-failed = 書き出せません: { $reason }
markup-export-same-file = 別の名前を選んでください。元のファイルは上書きしません
markup-export-encrypted = PDF が暗号化されています。代わりに PNG で書き出してください。
markup-export-damaged = PDF の構造が壊れています。代わりに PNG で書き出してください。
sheets-title = 図面を比較
sheets-other = 別の図面…
sheets-other-named = 比較先: { $name }
sheets-page = { $page } / { $count } ページ
sheets-auto = 位置合わせ
sheets-pick = 2 点を指定
sheets-overlay = 重ね合わせ
sheets-difference = 差分
sheets-blend = フェード
sheets-blink = 点滅
sheets-slider = 別の図面 { PERCENT($percent) }
sheets-clouds = { $count ->
    [0] 変更なし
   *[other] { $count } 件の変更を雲マークで囲む
}
sheets-done = 完了
sheets-choose = 比較する図面を選んでください。
sheets-reading = 図面を読み込み中…
sheets-aligning = 位置合わせ中…
sheets-aligned = 位置合わせ完了: 尺度 { $scale }、回転 { $angle }°、誤差 { $error } px 以内。{ $changes ->
    [0] 変更は見つかりませんでした。
   *[other] 変更 { $changes } 件。
}
sheets-align-failed = 自動で位置合わせできませんでした。2 点を指定してください。
sheets-pick-this = 点 { $n }: この図面上でクリックします。
sheets-pick-other = 点 { $n }: 別の図面上で同じ点をクリックします。
sheets-pick-same = 2 点が同じです。もう一度 2 点を指定してください。
sheets-read-failed = 図面を読み込めません: { $reason }
sheets-failed = この手順に失敗しました。やり直すか、別の図面を選んでください。
sheets-legend-this = この図面のみ
sheets-legend-other = 別の図面のみ
sheets-legend-both = 両方
sheets-cloud-text = 変更
sheets-clouds-added = { $count ->
    [0] 雲マークは追加されませんでした
   *[other] { $count } 個の雲マークをマークアップに追加しました
}
sheets-opened = 図面を比較
sheets-closed = 比較を終了しました
markup-ask-new-line = Shift+Enter: 改行

## Settings › Drawings

# machine-drafted, needs native review
settings-section-drawings = 図面
settings-drawings-shx-folders = SHX フォントのフォルダー
settings-drawings-shx-folders-help = DXF・DWG の文字に使う SHX フォントのフォルダー。; で区切ります。最初に図面と同じフォルダーを探します。
# A folder in the SHX font folders setting that does not exist or is not a folder.
settings-error-shx-folder = フォルダーではありません：{ $folder }
