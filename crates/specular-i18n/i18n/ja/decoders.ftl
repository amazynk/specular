# machine-drafted, needs native review
## Optional decoders: other vendors' free software that opens a few formats (Blackmagic RAW,
## DWG), used only when the user installed it. Specular never bundles or downloads it.
## $format is the format ("Blackmagic RAW", "DWG"), $decoder the decoder's own name
## ("Blackmagic RAW SDK", "ODA File Converter"), $vendor its maker ("Blackmagic Design",
## "Open Design Alliance"): names, left as they are.

## Settings › Optional decoders. The switches and paths are shown in the panel below
## "Look again", not as rows of their own.

settings-section-decoders = オプションのデコーダー
settings-decoders-look-again = 再検索
settings-decoders-look-again-help = その後にインストールされたデコーダーを探します。
    { -app-name } はウィンドウが前面に来るたびにも探します。
settings-decoders-braw = Blackmagic RAW SDK を使用
settings-decoders-braw-help = インストールされていれば、Blackmagic RAW クリップをこれで開きます。
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = 空欄にすると、インストーラーが置く場所から探します。
settings-decoders-dwg = ODA File Converter を使用
settings-decoders-dwg-help = インストールされていれば、DWG 図面をこれで開きます。
    図面のコピーを専用のフォルダーで変換し、そのフォルダーは後で削除します。
settings-decoders-dwg-path = ODA File Converter プログラム
settings-decoders-dwg-path-help = 空欄にすると、インストーラーが置く場所から探します。
settings-decoders-occt = OpenCASCADE を使用
settings-decoders-occt-help = インストールされていれば、STEP と IGES のモデルをこれで開きます。
settings-decoders-occt-path = OpenCASCADE ライブラリ
settings-decoders-occt-path-help = 空欄にすると、システムがライブラリを置く場所から探します。
# machine-drafted, needs native review
settings-decoders-czi = libCZI を使用
settings-decoders-czi-help = インストールされていれば、Zeiss CZI ファイルをこれで開きます。
    { -app-name } のサンドボックス化された補助プロセスで動作し、ほかのファイルにはアクセスしません。
settings-decoders-czi-path = libCZI ライブラリのフォルダー
settings-decoders-czi-path-help = 空欄にすると、システムがライブラリを置く場所から探します。

## The panel: one block per decoder.

decoders-intro = 一部の形式は、開発元の無料ソフトウェアで開きます。
    一度インストールすれば、{ -app-name } が見つけて使います。{ -app-name } がダウンロードすることはありません。
decoders-keywords = オプション デコーダー 開発元 ソフトウェア コーデック プラグイン デコーダーが必要 optional decoders vendor SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI needs decoder
# A decoder's row: "Blackmagic RAW (.braw)". The tick box before it switches it on and off.
decoders-row = { $format } ({ $extensions })
# The row's value.
decoders-state-found = 検出済み
decoders-state-off = オフ
decoders-state-not-found = 未インストール
decoders-state-broken = 使用できません
decoders-state-refused = 対応できません
decoders-state-planned = まだ使えません
decoders-state-checking = 検索中…
# The row's lines.
decoders-uses = { $vendor } の { $decoder } で開きます。
decoders-not-found = 無料です。インストールしてから「再検索」を押してください。
decoders-off = オフ: これらのファイルは開きません。
decoders-version = バージョン { $version }。
decoders-r3d-why = RED のライセンスでは、RED 独自の条件のもとで同社の SDK を使って作られたプログラムしか R3D ファイルを読めません。
    { -app-name } はその条件を受け入れられないため、R3D ファイルは開きません。
decoders-ecw-why = Hexagon のライセンスでは、ECW SDK を使えるのはそれを同梱するプログラムだけで、ご自分でインストールしたコピーは使えません。
    そのため ECW ファイルは開けません。ECW SDK で作られた JPEG 2000 ファイルは開けます。
decoders-mrsid-why = MrSID SDK のライセンスでは、使えるのはそれを同梱するプログラムだけで、ご自分でインストールしたコピーは使えません。
    そのため MrSID ファイルは開けません。
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = DJI の SDK のライセンスでは、使えるのはそれを同梱するプログラムだけで、ご自分でインストールしたコピーは使えません。写真とメタデータは開けます。
decoders-occt-why = STEP と IGES のソリッドには OpenCASCADE が必要ですが、このビルドの { -app-name } では使えません。
# Rows with one button each, under a decoder.
decoders-get-row = 開発元のダウンロードページ
decoders-get = 入手
decoders-locate-row = 別の場所にインストール済み?
decoders-locate = 場所を指定…
decoders-forget-row = 手動で選択したものです。
decoders-forget = 消去…
# Asked before Forget… does anything.
decoders-forget-ask = 手動で選択した { $decoder } の指定を消去しますか?
decoders-forget-detail = { -app-name } は再び自動で探します。ディスク上のものは何も削除されません。
decoders-forget-ok = 消去
# The title of the Locate… file dialog.
decoders-locate-title = { $decoder } の場所を指定
# Toasts.
decoders-locate-wrong = これは { $decoder } ではありません。
decoders-located = 選択した { $decoder } を使用しています。
decoders-forgotten = { -app-name } は再び自動で { $decoder } を探します。
decoders-still-missing = { $decoder } がまだ見つかりません。
decoders-now-found = { $decoder } が見つかりました。
decoders-looked = デコーダーを再検索しました。

## The card shown in place of a file that needs one. $file is the file's name.

decoders-card-title = { $format } には { $vendor } の無料デコーダーが必要です
decoders-card-what = { $file } は { $decoder } で開きます。無料です。
decoders-card-how = インストールしたら戻ってきてください。{ -app-name } が自動で見つけます。
decoders-card-get = 入手
decoders-card-installed = インストールしました
decoders-card-locate = 場所を指定…

## The gallery: the badge on such a file's thumbnail, and a line of its tooltip.

gallery-badge-needs-decoder = デコーダーが必要
gallery-needs-decoder = { $vendor } の無料の { $decoder } で開きますが、インストールされていません。

## Why a file did not open, after "Can't open FILE:".

error-reason-needs-decoder = { $format } ファイルには { $vendor } の無料の { $decoder } が必要ですが、インストールされていません。
