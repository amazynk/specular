# machine-drafted, needs native review
# Specular — スタート画面 (ファイルを指定せずに Specular を開いたときと、最後の画像を閉じたときに表示)、
# 「試してみる」カード、一度だけ表示されるヒント。日本語。

## スタート画面

welcome-title = { -app-name }
welcome-tagline = 写真と、技術・科学・医療画像のための、高速で忠実なビューアーです。
welcome-open = ファイルを開く…
welcome-open-about = 画像、文書、データファイル。
welcome-browse = フォルダーを参照…
welcome-browse-about = フォルダーを選んでギャラリーで表示します。
welcome-pictures = あなたの写真
welcome-pictures-about = ライブラリ内のすべての写真を、日付と場所ごとに。
welcome-pictures-setup-title = ライブラリを作成…
welcome-pictures-setup = { $folder } フォルダーから始めます。
welcome-pictures-ask = { $folder } フォルダーをライブラリに追加しますか?
welcome-pictures-add = フォルダーを追加
welcome-drop = このウインドウのどこにでもファイルやフォルダーをドロップできます。
welcome-recent = 最近のフォルダー
welcome-recent-name = { $name } ({ $parent } 内)
welcome-tips = 試してみる
welcome-tips-hide = このヒントを隠す
welcome-tips-hidden = ヒントを隠しました。コマンドパレットの「ようこそを表示」で戻せます。

## 試してみる: 各カードに名前と 1 行の説明。名前の横のキーはユーザー自身の割り当てです。

welcome-tip-gallery = ギャラリー
welcome-tip-gallery-about = フォルダー全体をひと目で。
welcome-tip-compare = 比較
welcome-tip-compare-about = ギャラリーで 2〜4 枚の画像を選ぶと、並べて表示できます。
welcome-tip-cull = 選別モード
welcome-tip-cull-about = 撮影した写真の採用・不採用・評価をそれぞれキー 1 つで。
welcome-tip-slideshow = スライドショー
welcome-tip-slideshow-about = フォルダーを全画面で 1 枚ずつ表示します。
welcome-tip-palette = コマンドパレット
welcome-tip-palette-about = あらゆるコマンドや設定を名前で検索します。
welcome-tip-studies = 検査
welcome-tip-studies-about = DICOM ディスクやフォルダーの患者、検査、シリーズ。
welcome-tip-dicom-servers = DICOMweb サーバー
welcome-tip-dicom-servers-about = PACS の検査を参照して開きます。
welcome-tip-dicom-tags = DICOM タグ
welcome-tip-dicom-tags-about = 画像のすべてのタグを検索できます。
welcome-tip-window-level = ウィンドウ/レベル
welcome-tip-window-level-about = ドラッグして DICOM 画像のコントラストを調整します。
welcome-tip-omero-servers = OMERO サーバー
welcome-tip-omero-servers-about = OMERO と IDR の顕微鏡プロジェクトを参照します。
welcome-tip-stretch = ストレッチ
welcome-tip-stretch-about = 天文・科学データのかすかな細部を浮かび上がらせます。
welcome-tip-drawing-layers = 図面の画層
welcome-tip-drawing-layers-about = 技術図面の画層を表示・非表示にします。
welcome-tip-needs-image = まず画像を開いてから、{ $key } を押してください。
welcome-tip-needs-gallery = ギャラリーで 2〜4 枚の画像を選んでから、{ $key } を押してください。
welcome-tip-key = ショートカット: { $key }

## 一度だけのヒント: それぞれ一度だけ控えめに、起動ごとに最大 1 つ。設定 › 一般 ›
## ヒントを表示 ですべてオフにできます。

hint-gallery = { $key } を押すと、このフォルダーをギャラリーで表示します。
hint-gallery-first = 2〜4 枚の画像を選び、{ $compare } で比較、{ $cull } でフォルダーを選別します。
hint-raw = RAW 写真のフォルダーです。{ $key } を押して選別できます (採用・不採用・評価)。
hint-pages =
    { $count ->
       *[other] このファイルは { $count } ページあります。{ $next } と { $prev } でページを移動します。
    }
hint-turn-off = ヒントをオフにする
hint-turned-off = ヒントをオフにしました。設定 › 一般 › ヒントを表示 で再びオンにできます。

## コマンドパレット

palette-show-welcome = ようこそを表示
palette-show-welcome-help = 開く、参照、あなたの写真、試してみる項目があるスタート画面。
palette-close = 閉じる
palette-close-help = 画像またはフォルダーを閉じてスタート画面に戻ります。スタート画面では終了します。

## 初回起動 (プロファイルの質問の下)

profile-on-start = ファイルを指定せずに { -app-name } を開いたとき:
profile-skip = スキップ
