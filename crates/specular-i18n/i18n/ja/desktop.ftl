# デスクトップとの統合: 設定 → 統合、壁紙に設定。

settings-section-integration = 統合
settings-integration-default-viewer = 既定の画像ビューア
settings-integration-default-viewer-help = ダブルクリックした画像を { -app-name } で開きます。削除すると、以前のビューアに戻ります。
# machine-drafted, needs native review
settings-integration-extra-default-types = その他のファイル形式
settings-integration-extra-default-types-help = ダブルクリックで { -app-name } で開くその他のファイル。「既定の画像ビューア」で選びます。
settings-integration-thumbnailer = ファイルマネージャーのサムネイル
settings-integration-thumbnailer-help = JPEG XL、HEIC、カメラの RAW ファイルなど、ファイルマネージャーが自分では読めない形式のプレビューを表示できるようにします。
settings-integration-single-instance = D-Bus による単一インスタンス
settings-integration-single-instance-help = ファイルマネージャー、ランチャー、ターミナルから開いたファイルを、実行中の { -app-name } のウィンドウで表示します。この後に開くウィンドウから有効になります。
settings-integration-file-manager-actions = ファイルマネージャーのアクション
settings-integration-file-manager-actions-help = Nautilus、Dolphin、Thunar、Nemo のコンテキストメニューに「開く」「比較」「壁紙に設定」を追加します。
settings-integration-search-provider = デスクトップ検索
# machine-drafted, needs native review
settings-integration-search-provider-help = GNOME と KDE の検索から、ライブラリの写真を探せるようにします。語はライブラリの検索バーと同じように使えます。
# machine-drafted, needs native review
settings-integration-url-scheme = specular:// リンクを開く
settings-integration-url-scheme-help = ブラウザーや他のプログラムの specular:///home/me/photo.jpg のようなリンクを { -app-name } で開きます。開くのはこのコンピューター上のファイルのみで、通常と異なるものは確認後に開きます。

integration-state-on = オン
integration-state-off = オフ
integration-state-partial = 一部だけ設定されています。もう一度適用すると修復されます。
integration-state-unavailable = このデスクトップでは使用できません
integration-state-sandboxed = サンドボックス (Flatpak) 内では使用できません
integration-state-no-bus = 使用できません: D-Bus セッションがありません
integration-state-no-file-manager = 使用できません: 対応するファイルマネージャーが見つかりません
integration-state-no-search = 使用できません: GNOME Shell も KRunner も見つかりません
integration-state-working = 処理中…
integration-state-details = { $state } · { $details }
integration-detail-types = 画像の種類: { $total } 中 { $count }
integration-detail-thumbnail-types = 画像の種類: { $count }
integration-apply = 適用
integration-remove = 削除
integration-done-on = 「{ $option }」をオンにしました
integration-done-off = 「{ $option }」をオフにしました
integration-failed = 「{ $option }」を変更できませんでした: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = デスクトップ検索をオフにしますか？
integration-search-remove-detail = { $desktops } の検索から { -app-name } を外します。ライブラリと写真はそのまま残ります。
search-provider-open-folder = フォルダーを開く

integration-label-open = { -app-name } で開く
integration-label-compare = { -app-name } で比較
integration-label-wallpaper = 壁紙に設定
integration-label-generic-name = 画像ビューア
integration-label-comment = 大きな写真もすぐに開いてめくれます

palette-set-wallpaper = 壁紙に設定
wallpaper-set = 壁紙を設定しました ({ $method })
wallpaper-failed = 壁紙を設定できませんでした: { $reason }

# machine-drafted, needs native review
url-invalid = この { -app-name } リンクを開けません: { $reason }
url-not-found = このリンクが示すファイルは存在しません
url-remote-refused = { -app-name } はこのコンピューター上のファイルのみ開きます（{ $source } は開きません）
url-confirm-files = { $more ->
    [0] 「{ $file }」
   *[other] 「{ $file }」ほか { $more } 件
}
url-confirm-title = { $reason ->
    [dots] { url-confirm-files }を開きますか？ リンクは「..」フォルダーを経由しています。
    [network] { url-confirm-files }を開きますか？ ネットワーク共有またはデバイス上にあります。
   *[hidden] { url-confirm-files }を開きますか？ 名前に不可視文字または文字方向の制御文字が含まれています。
}
url-confirm-open = 開く

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = 新しいウィンドウ
integration-label-library = ライブラリ
integration-label-slideshow = ピクチャのスライドショー
settings-integration-recent-files = デスクトップの最近使ったファイル
settings-integration-recent-files-help = 開いた画像がファイルマネージャーの「最近使用したもの」やデスクトップの最近使ったファイルの一覧に表示されます。削除すると { -app-name } の項目がこれらの一覧から取り除かれます。
integration-detail-recent = ファイル: { $count }
# machine-drafted, needs native review
integration-extra-title = その他のファイル形式
# machine-drafted, needs native review
integration-extra-intro = { -app-name } は画像以外に、これらのファイルもダブルクリックで開けます。自分でチェックしない限り何も選ばれません。これらを編集するプログラムは「プログラムから開く」に残り、チェックを外すと以前のプログラムに戻ります。Blackmagic RAW、DWG、STEP、IGES、Zeiss CZI には開発元の無料ソフトウェアも必要です。「設定 › オプションのデコーダー」を参照してください。
integration-extra-chosen = { $count ->
    [0] 未選択
   *[other] { $count } 種類を選択
}
integration-extra-count = { $on } / { $total }
integration-extra-none-yet = まだありません
integration-extra-keywords = ファイル形式 フォーマット 関連付け ダブルクリック プログラムから開く エディター 科学 医療 CAD GIS 3D 分子 動画 ビデオ SVG PDF
integration-extra-group-editors = 編集ソフトのファイル
# machine-drafted, needs native review
integration-extra-group-editors-help = ペイント・ドローソフトの文書: Krita、GIMP、Paint.NET、PaintShop Pro、Procreate、Affinity、OpenRaster、Illustrator、CorelDRAW、Clip Studio Paint、Sketch、Figma。ダブルクリックで画像を表示します。編集は作成元のプログラムで行います。
integration-extra-group-science = 科学
integration-extra-group-science-help = 測定機器や研究ソフトのファイル: サーモカメラ、顕微鏡、表面測定、HDF5、NetCDF、地震・気象データ。
integration-extra-group-medical = 医療
integration-extra-group-medical-help = 医用画像と脳画像: DICOM、NIfTI、MINC、NRRD、トラクトグラフィ、脳表面。
integration-extra-group-cad-gis = CAD と GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = プロッタ図面、技術イラスト、チップレイアウト、地図: HP-GL、CGM、GDSII、シェープファイル、GeoPackage、KML、GPX、GeoJSON、標高タイル。DWG は無料の ODA File Converter で、STEP と IGES は OpenCASCADE で開きます。
integration-extra-group-models = 3D と分子
integration-extra-group-models-help = 3D モデル、建築モデル、点群、分子: STL、OBJ、glTF、PLY、3MF、X3D、IFC、LAS、E57、PDB、CIF など。
integration-extra-group-videos = 動画
# machine-drafted, needs native review
integration-extra-group-videos-help = 動画: MP4、MOV、M4V、3GP、3G2、MKV、WebM、AVCHD（MTS、M2TS）、OGV、IVF、MXF。Blackmagic RAW は無料の Blackmagic RAW SDK で開きます。
integration-extra-group-other-images = SVG とその他の画像
# machine-drafted, needs native review
integration-extra-group-other-images-help = 通常は他のプログラムに任せる画像や文書: SVG 図面、PDF、XPS、DjVu、EPS、WMF、EMF、Visio、OpenDocument、WordPerfect Graphics、EPUB 書籍とフォント、および MNG、FLIC、アニメーションカーソルなどのアニメーション。
integration-extra-group-text = テキスト・表・字幕
integration-extra-group-text-help = プレーンテキストとログ、NFO、Markdown、CSV と TSV の表、RTF、字幕ファイル（SRT、WebVTT、ASS、SSA、MicroDVD、PGS、LRC、TTML、SCC）。
