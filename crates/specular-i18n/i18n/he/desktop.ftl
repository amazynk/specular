# שילוב עמוק בשולחן העבודה: הגדרות ← שילוב, הגדרה כרקע.

settings-section-integration = שילוב
settings-integration-default-viewer = מציג התמונות כברירת מחדל
settings-integration-default-viewer-help = פתיחת תמונות ב־{ -app-name } בלחיצה כפולה. הסרה משחזרת את המציג הקודם.
# machine-drafted, needs native review
settings-integration-extra-default-types = סוגי קבצים נוספים
settings-integration-extra-default-types-help = סוגי קבצים נוספים שייפתחו ב־{ -app-name } בלחיצה כפולה, לבחירה תחת „מציג התמונות כברירת מחדל”.
settings-integration-thumbnailer = תמונות ממוזערות במנהל הקבצים
settings-integration-thumbnailer-help = מנהלי קבצים יציגו תצוגה מקדימה גם לפורמטים שהם לא יודעים לקרוא, כמו JPEG XL‏, HEIC וקובצי RAW של מצלמות.
settings-integration-single-instance = מופע יחיד דרך D-Bus
settings-integration-single-instance-help = קבצים שנפתחים ממנהל הקבצים, ממשגר או מהמסוף נשלחים לחלון { -app-name } הפתוח. חל על חלונות שייפתחו לאחר מכן.
settings-integration-file-manager-actions = פעולות במנהל הקבצים
settings-integration-file-manager-actions-help = הוספת „פתיחה”, „השוואה” ו„הגדרה כרקע” לתפריטי ההקשר של Nautilus‏, Dolphin‏, Thunar ו־Nemo.
settings-integration-search-provider = חיפוש בשולחן העבודה
# machine-drafted, needs native review
settings-integration-search-provider-help = חיפוש תמונות מהספרייה שלך בחיפוש של GNOME ו־KDE. המילים פועלות כמו בשורת החיפוש של הספרייה.
# machine-drafted, needs native review
settings-integration-url-scheme = פתיחת קישורי specular://
settings-integration-url-scheme-help = קישורים כמו specular:///home/me/photo.jpg בדפדפן או בתוכנה אחרת נפתחים ב־{ -app-name }. נפתחים רק קבצים במחשב הזה, וקבצים חריגים רק לאחר אישורך.

integration-state-on = פעיל
integration-state-off = כבוי
integration-state-partial = מוגדר חלקית. יש להחיל שוב כדי לתקן.
integration-state-unavailable = לא זמין בשולחן עבודה זה
integration-state-sandboxed = לא זמין בתוך ארגז חול (Flatpak)
integration-state-no-bus = לא זמין: אין הפעלת D-Bus
integration-state-no-file-manager = לא זמין: לא נמצא מנהל קבצים נתמך
integration-state-no-search = לא זמין: לא נמצאו GNOME Shell או KRunner
integration-state-working = מתבצע…
integration-state-details = { $state } · { $details }
integration-detail-types = סוגי תמונות: { $count } מתוך { $total }
integration-detail-thumbnail-types = סוגי תמונות: { $count }
integration-apply = החלה
integration-remove = הסרה
integration-done-on = „{ $option }” פעיל
integration-done-off = „{ $option }” כבוי
integration-failed = לא ניתן לשנות את „{ $option }”: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = לכבות את החיפוש בשולחן העבודה?
integration-search-remove-detail = { -app-name } יוסר מהחיפוש של { $desktops }. הספרייה והתמונות שלך נשארות.
search-provider-open-folder = פתיחת התיקייה

integration-label-open = פתיחה ב־{ -app-name }
integration-label-compare = השוואה ב־{ -app-name }
integration-label-wallpaper = הגדרה כרקע
integration-label-generic-name = מציג תמונות
integration-label-comment = פתיחה ודפדוף מיידיים בתמונות גדולות

palette-set-wallpaper = הגדרה כרקע
wallpaper-set = הרקע הוגדר ({ $method })
wallpaper-failed = לא ניתן להגדיר את הרקע: { $reason }

# machine-drafted, needs native review
url-invalid = לא ניתן לפתוח את קישור { -app-name } הזה: { $reason }
url-not-found = הקבצים שהקישור מציין אינם קיימים
url-remote-refused = { -app-name } פותח רק קבצים במחשב הזה, לא { $source }
url-confirm-files = { $more ->
    [0] „{ $file }”
   *[other] „{ $file }” ועוד { $more }
}
url-confirm-title = { $reason ->
    [dots] לפתוח את { url-confirm-files }? הקישור מגיע אליו דרך תיקיות „..”.
    [network] לפתוח את { url-confirm-files }? הוא נמצא בשיתוף רשת או בהתקן.
   *[hidden] לפתוח את { url-confirm-files }? שמו מכיל תווים בלתי נראים או תווי כיוון טקסט.
}
url-confirm-open = פתיחה

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = חלון חדש
integration-label-library = ספרייה
integration-label-slideshow = מצגת של תמונות
settings-integration-recent-files = קבצים אחרונים בשולחן העבודה
settings-integration-recent-files-help = תמונות שנפתחות מופיעות תחת „אחרונים” במנהל הקבצים וברשימות הקבצים האחרונים של שולחן העבודה. הסרה מוציאה שוב את הרשומות של { -app-name } מרשימות אלה.
integration-detail-recent = קבצים: { $count }
# machine-drafted, needs native review
integration-extra-title = סוגי קבצים נוספים
# machine-drafted, needs native review
integration-extra-intro = מלבד תמונות, { -app-name } יכול לפתוח גם את הקבצים האלה בלחיצה כפולה. שום דבר לא מסומן עד שתסמנו אותו. התוכנות שעורכות את הקבצים נשארות ב„פתיחה באמצעות”, וביטול הסימון מחזיר את הסוג לתוכנה שהייתה לו קודם. בנוסף, Blackmagic RAW, DWG, STEP, IGES ו־Zeiss CZI דורשים תוכנה חינמית של היצרנים שלהם: ראו הגדרות › מפענחים אופציונליים.
integration-extra-chosen = { $count ->
    [0] לא נבחר דבר
    [one] נבחר סוג אחד
    [two] נבחרו שני סוגים
   *[other] נבחרו { $count } סוגים
}
integration-extra-count = { $on } מתוך { $total }
integration-extra-none-yet = עדיין אין
integration-extra-keywords = סוגי קבצים פורמטים שיוך לחיצה כפולה פתיחה באמצעות עורך מדע רפואה CAD GIS תלת־ממד 3D מולקולות וידאו SVG PDF
integration-extra-group-editors = קובצי עורכים
# machine-drafted, needs native review
integration-extra-group-editors-help = מסמכים של תוכנות ציור ושרטוט: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch ו־Figma. לחיצה כפולה מציגה את התמונה; העריכה נשארת בתוכנה שיצרה אותה.
integration-extra-group-science = מדע
integration-extra-group-science-help = קבצים ממכשירים ומתוכנות מחקר: מצלמות תרמיות, מיקרוסקופים, מדידות משטח, HDF5, NetCDF, נתונים סייסמיים ונתוני מזג אוויר.
integration-extra-group-medical = רפואה
integration-extra-group-medical-help = סריקות רפואיות והדמיית מוח: DICOM, NIfTI, MINC, NRRD, טרקטוגרפיה ומשטחי מוח.
integration-extra-group-cad-gis = CAD ו־GIS
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = שרטוטי פלוטר, איורים טכניים, תכנוני שבבים ומפות: HP-GL, CGM, GDSII, shapefile, GeoPackage, KML, GPX, GeoJSON ואריחי גובה. DWG נפתח בעזרת ODA File Converter החינמי, ו־STEP ו־IGES בעזרת OpenCASCADE.
integration-extra-group-models = תלת־ממד ומולקולות
integration-extra-group-models-help = מודלים תלת־ממדיים, מודלי בניינים, ענני נקודות ומולקולות: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF ועוד.
integration-extra-group-videos = סרטונים
# machine-drafted, needs native review
integration-extra-group-videos-help = סרטונים: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF ו־MXF. Blackmagic RAW נפתח בעזרת Blackmagic RAW SDK החינמי.
integration-extra-group-other-images = SVG ותמונות אחרות
# machine-drafted, needs native review
integration-extra-group-other-images-help = תמונות ומסמכים שבדרך כלל נפתחים בתוכנות אחרות: שרטוטי SVG, PDF, XPS, DjVu, EPS, WMF ו־EMF, Visio, OpenDocument, WordPerfect Graphics, ספרי EPUB וגופנים, ואנימציות כמו MNG, FLIC וסמני עכבר מונפשים.
integration-extra-group-text = טקסט, טבלאות וכתוביות
integration-extra-group-text-help = טקסט פשוט ויומנים, NFO, Markdown, טבלאות CSV ו־TSV, RTF וקובצי כתוביות: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML ו־SCC.
