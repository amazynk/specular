# machine-drafted, needs native review
## Optional decoders: other vendors' free software that opens a few formats (Blackmagic RAW,
## DWG), used only when the user installed it. Specular never bundles or downloads it.
## $format is the format ("Blackmagic RAW", "DWG"), $decoder the decoder's own name
## ("Blackmagic RAW SDK", "ODA File Converter"), $vendor its maker ("Blackmagic Design",
## "Open Design Alliance"): names, left as they are.

## Settings › Optional decoders. The switches and paths are shown in the panel below
## "Look again", not as rows of their own.

settings-section-decoders = מפענחים אופציונליים
settings-decoders-look-again = חיפוש חוזר
settings-decoders-look-again-help = חיפוש מפענחים שהותקנו מאז.
    { -app-name } מחפש גם בכל פעם שהחלון שלו עובר לחזית.
settings-decoders-braw = שימוש ב־Blackmagic RAW SDK
settings-decoders-braw-help = קטעי Blackmagic RAW נפתחים בעזרתו כשהוא מותקן.
settings-decoders-braw-path = Blackmagic RAW SDK
settings-decoders-braw-path-help = יש להשאיר ריק כדי למצוא אותו במקום שבו תוכנת ההתקנה שלו שמה אותו.
settings-decoders-dwg = שימוש ב־ODA File Converter
settings-decoders-dwg-help = שרטוטי DWG נפתחים בעזרתו כשהוא מותקן.
    עותק של השרטוט מומר בתיקייה פרטית, שנמחקת לאחר מכן.
settings-decoders-dwg-path = התוכנה ODA File Converter
settings-decoders-dwg-path-help = יש להשאיר ריק כדי למצוא אותה במקום שבו תוכנת ההתקנה שלה שמה אותה.
settings-decoders-occt = שימוש ב־OpenCASCADE
settings-decoders-occt-help = מודלים של STEP ו־IGES נפתחים בעזרתו כשהוא מותקן.
settings-decoders-occt-path = ספריות OpenCASCADE
settings-decoders-occt-path-help = יש להשאיר ריק כדי למצוא אותן במקום שבו המערכת שומרת ספריות.
# machine-drafted, needs native review
settings-decoders-czi = שימוש ב־libCZI
settings-decoders-czi-help = קובצי Zeiss CZI נפתחים בעזרתו כשהוא מותקן.
    הוא פועל בתהליך העזר המבודד של { -app-name }, ללא גישה לקבצים האחרים שלך.
settings-decoders-czi-path = תיקיית הספרייה libCZI
settings-decoders-czi-path-help = יש להשאיר ריק כדי למצוא אותה במקום שבו המערכת שומרת ספריות.

## The panel: one block per decoder.

decoders-intro = פורמטים מסוימים נפתחים בעזרת תוכנה חינמית של היצרנים שלהם.
    מספיק להתקין אותה פעם אחת: { -app-name } ימצא אותה וישתמש בה. הוא אף פעם לא מוריד אותה בעצמו.
decoders-keywords = מפענחים אופציונליים יצרן תוכנה קודק תוסף נדרש מפענח optional decoders vendor SDK codec plugin Blackmagic RAW BRAW RED R3D DWG ODA File Converter OpenCASCADE STEP IGES Zeiss CZI libCZI needs decoder
# A decoder's row: "Blackmagic RAW (.braw)". The tick box before it switches it on and off.
decoders-row = { $format } ({ $extensions })
# The row's value.
decoders-state-found = נמצא
decoders-state-off = כבוי
decoders-state-not-found = לא מותקן
decoders-state-broken = לא שמיש
decoders-state-refused = לא אפשרי
decoders-state-planned = עדיין לא זמין
decoders-state-checking = בחיפוש…
# The row's lines.
decoders-uses = נפתח בעזרת { $decoder } של { $vendor }.
decoders-not-found = זה בחינם. יש להתקין, ואז ללחוץ על „חיפוש חוזר”.
decoders-off = כבוי: הקבצים האלה לא נפתחים.
decoders-version = גרסה { $version }.
decoders-r3d-why = הרישיון של RED מתיר לקרוא קובצי R3D רק לתוכנות שנבנו עם ה־SDK שלה, לפי התנאים של RED עצמה.
    { -app-name } לא יכול לקבל את התנאים האלה, ולכן קובצי R3D לא נפתחים.
decoders-ecw-why = הרישיון של Hexagon מתיר שימוש ב־ECW SDK רק לתוכנות שמגיעות איתו, לא לעותק שהתקנת בעצמך.
    לכן קובצי ECW לא נפתחים. קובצי JPEG 2000 שנוצרו ב־ECW SDK כן נפתחים.
decoders-mrsid-why = הרישיון של MrSID SDK מתיר שימוש בו רק לתוכנות שמגיעות איתו, לא לעותק שהתקנת בעצמך.
    לכן קובצי MrSID לא נפתחים.
# machine-drafted, needs native review
# DJI radiometric JPEGs (drone thermal cameras): the photo and metadata open; the
# temperatures would need DJI's SDK.
decoders-dji-thermal-why = הרישיון של ה־SDK של DJI מתיר שימוש בו רק לתוכנות שמגיעות איתו, לא לעותק שהתקנת בעצמך. התמונה והמטא־נתונים שלה עדיין נפתחים.
decoders-occt-why = גופי STEP ו־IGES דורשים את OpenCASCADE, וגרסה זו של { -app-name } לא יכולה להשתמש בו.
# Rows with one button each, under a decoder.
decoders-get-row = דף ההורדה של היצרן
decoders-get = הורדה
decoders-locate-row = מותקן במקום אחר?
decoders-locate = איתור…
decoders-forget-row = נבחר ידנית.
decoders-forget = לשכוח…
# Asked before Forget… does anything.
decoders-forget-ask = לשכוח את { $decoder } שנבחר ידנית?
decoders-forget-detail = { -app-name } יחזור לחפש אותו בעצמו. שום דבר לא יוסר מהדיסק.
decoders-forget-ok = לשכוח
# The title of the Locate… file dialog.
decoders-locate-title = איתור { $decoder }
# Toasts.
decoders-locate-wrong = זה לא { $decoder }.
decoders-located = נעשה שימוש ב־{ $decoder } שנבחר ידנית.
decoders-forgotten = { -app-name } שוב מחפש את { $decoder } בעצמו.
decoders-still-missing = עדיין לא נמצא { $decoder }.
decoders-now-found = נמצא { $decoder }.
decoders-looked = בוצע חיפוש חוזר של מפענחים.

## The card shown in place of a file that needs one. $file is the file's name.

decoders-card-title = נדרש המפענח החינמי של { $vendor } עבור { $format }
decoders-card-what = הקובץ { $file } נפתח בעזרת { $decoder }. זה בחינם.
decoders-card-how = יש להתקין ולחזור לכאן: { -app-name } ימצא אותו בעצמו.
decoders-card-get = הורדה
decoders-card-installed = התקנתי
decoders-card-locate = איתור…

## The gallery: the badge on such a file's thumbnail, and a line of its tooltip.

gallery-badge-needs-decoder = נדרש מפענח
gallery-needs-decoder = נפתח בעזרת { $decoder } החינמי של { $vendor }, שאינו מותקן.

## Why a file did not open, after "Can't open FILE:".

error-reason-needs-decoder = קובצי { $format } דורשים את { $decoder } החינמי של { $vendor }, שאינו מותקן.
