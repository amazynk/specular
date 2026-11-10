# Specular — גלריות ללקוחות: תיקיית HTML סטטית של תמונות, וקריאת הבחירות של הלקוח בחזרה.

# machine-drafted, needs native review

palette-client-gallery = גלריה ללקוח…
palette-client-gallery-help = כתיבת התמונות כתיקיית דף אינטרנט ללקוח, עם בחירות שהלקוח יכול לשלוח בחזרה.
palette-import-client-picks = ייבוא בחירות הלקוח…
palette-import-client-picks-help = סימון התמונות שהלקוח בחר בגלריה שלו.

webgallery-heading = גלריה ללקוח
webgallery-subtitle = { $count ->
    [one] תמונה אחת
    [two] שתי תמונות
   *[other] { $count } תמונות
}
webgallery-title-label = כותרת
webgallery-title-default = הגהות
webgallery-watermark-label = טקסט סימן מים (ריק: ללא)
webgallery-source = תמונות
webgallery-source-current = התמונה הזו
webgallery-source-selection = הבחירה
webgallery-source-album = האלבום ”{ $name }“
webgallery-source-search = תוצאות החיפוש
webgallery-source-folder = התיקייה הזו
webgallery-source-picks = מועדפות
webgallery-source-value = { $what } ({ $count })
webgallery-theme = ערכת עיצוב
webgallery-theme-light = בהירה
webgallery-theme-dark = כהה
webgallery-theme-minimal = מינימלית
webgallery-size = צלע ארוכה
webgallery-size-value = { $px } פיקסלים
webgallery-format = תבנית
webgallery-captions = מתחת לכל תמונה
webgallery-captions-off = כלום
webgallery-captions-name = שם הקובץ
webgallery-captions-title = כותרת
webgallery-captions-all = שם הקובץ, כותרת וכיתוב
webgallery-exif = הצגת המצלמה והחשיפה
webgallery-location = שמירת המיקום (GPS) בתמונות
webgallery-note-private = אי אפשר להגן על גלריה סטטית בסיסמה.
webgallery-note-share = כל מי שיש לו את התיקייה או את הכתובת שלה רואה כל תמונה. שתפו את הקישור רק עם הלקוח, או השתמשו בבקרת הגישה של שירות האחסון.
webgallery-note-picks = הלקוחות בוחרים בלב. הבחירות נשארות בדפדפן שלהם עד שהם שולחים לכם את הרשימה.
webgallery-write = כתיבת הגלריה…
webgallery-busy = כותב { $done } מתוך { $total }…
webgallery-stop = עצירה
webgallery-done = { $count ->
    [one] הגלריה נכתבה: תמונה אחת ב־{ $folder }
    [two] הגלריה נכתבה: שתי תמונות ב־{ $folder }
   *[other] הגלריה נכתבה: { $count } תמונות ב־{ $folder }
}
webgallery-done-failed = { $failed ->
    [one] לא ניתן היה לכתוב תמונה אחת.
    [two] לא ניתן היה לכתוב שתי תמונות.
   *[other] לא ניתן היה לכתוב { $failed } תמונות.
}
webgallery-stopped = הגלריה נעצרה. דבר לא נשמר.
webgallery-failed = הגלריה לא נכתבה: { $reason }
webgallery-nothing = לא ניתן היה לכתוב אף תמונה.

webgallery-page-count = { $count ->
    [one] תמונה אחת
    [two] שתי תמונות
   *[other] { $count } תמונות
}
webgallery-page-hint = הקישו על הלב בתמונות שאתם רוצים. הבחירות נשארות בדפדפן הזה עד שתשלחו אותן.
webgallery-page-picked = נבחרו: {"{"}n{"}"}
webgallery-page-pick = בחירה
webgallery-page-copy = העתקת הבחירות שלי
webgallery-page-copied = הועתק. הדביקו את הרשימה בהודעה.
webgallery-page-copy-by-hand = העתיקו את הרשימה המסומנת והדביקו אותה בהודעה.
webgallery-page-download-txt = הורדת picks.txt
webgallery-page-download-json = הורדת picks.json
webgallery-page-clear = ניקוי הבחירות
webgallery-page-clear-ask = לנקות את כל הבחירות שלכם בדפדפן הזה?
webgallery-page-no-storage = הדפדפן הזה לא ישמור את הבחירות שלכם. שלחו אותן לפני שתסגרו את הדף.
webgallery-page-noscript = לבחירה נדרש JavaScript. התמונות עדיין נפתחות.
webgallery-page-previous = הקודמת
webgallery-page-next = הבאה
webgallery-page-close = סגירה
webgallery-page-position = {"{"}i{"}"} מתוך {"{"}n{"}"}
webgallery-page-viewer = מציג תמונות
webgallery-page-picks = הבחירות שלכם

clientpicks-heading = ייבוא בחירות הלקוח
clientpicks-subtitle = { $count ->
    [one] הותאם מול תמונה אחת מוצגת
    [two] הותאם מול שתי התמונות המוצגות
   *[other] הותאם מול { $count } התמונות המוצגות
}
clientpicks-paste-label = שמות (הדביקו את הרשימה של הלקוח)
clientpicks-choose = בחירת קובץ בחירות…
clientpicks-from-file = מתוך { $name }
clientpicks-apply-as = סימון
clientpicks-as-pick = מועדפות
clientpicks-as-stars = { $stars ->
    [one] כוכב אחד
    [two] שני כוכבים
   *[other] { $stars } כוכבים
}
clientpicks-as-label = תווית { $label }
clientpicks-empty = הדביקו את הרשימה שהלקוח שלח, או בחרו את קובץ picks.txt או picks.json שלו.
clientpicks-matched = { $matched } מתוך { $total } שמות תואמים לתמונות כאן.
clientpicks-missing = לא נמצאו: { $names }
clientpicks-ambiguous = ביותר מתיקייה אחת, לא שונו: { $names }
clientpicks-more = { $names } ועוד { $more }
clientpicks-apply = החלה
clientpicks-applied = { $count ->
    [one] סומנה תמונה אחת.
    [two] סומנו שתי תמונות.
   *[other] סומנו { $count } תמונות.
}
clientpicks-unchanged = התמונות האלה כבר היו מסומנות.
clientpicks-read-failed = לא ניתן לקרוא את { $name }: { $reason }
