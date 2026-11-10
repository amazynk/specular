# The play queue: what plays next, repeat and shuffle, playlists (M3U, PLS, XSPF), the
# queue panel and the sound transport's buttons.

# machine-drafted, needs native review
cheatsheet-video-queue = תור, חזרה וערבוב
# machine-drafted, needs native review
palette-next-track = הפריט הבא
# machine-drafted, needs native review
palette-next-track-help = הפעלת הפריט הבא בתור, ובלי תור הקובץ הבא מאותו סוג בתיקייה.
# machine-drafted, needs native review
palette-prev-track = הפריט הקודם
# machine-drafted, needs native review
palette-prev-track-help = הפעלת הפריט הקודם. אחרי 3 שניות, הפריט הנוכחי מתחיל מההתחלה.
# machine-drafted, needs native review
palette-cycle-repeat = חזרה
# machine-drafted, needs native review
palette-cycle-repeat-help = חזרה כבויה, על הכול או על פריט אחד.
# machine-drafted, needs native review
palette-shuffle = ערבוב
# machine-drafted, needs native review
palette-shuffle-help = הפעלה בסדר אקראי, כל פריט פעם אחת בכל סבב.
# machine-drafted, needs native review
palette-play-queue = תור ההפעלה
# machine-drafted, needs native review
palette-play-queue-help = הצגת מה יופעל אחר כך: שינוי סדר, הסרה, שמירה.
# machine-drafted, needs native review
palette-queue-play-next = להפעיל אחר כך
# machine-drafted, needs native review
palette-queue-play-next-help = הפעלת הקובץ (בגלריה, הבחירה) אחרי הפריט שמתנגן.
# machine-drafted, needs native review
palette-queue-add = הוספה לתור
# machine-drafted, needs native review
palette-queue-add-help = הוספת הקובץ (בגלריה, הבחירה) לסוף התור.
# machine-drafted, needs native review
palette-save-queue = שמירת התור כרשימת השמעה…
# machine-drafted, needs native review
palette-save-queue-help = שמירת התור כרשימת השמעה מסוג M3U8 או XSPF.
# machine-drafted, needs native review
transport-prev-track = הפריט הקודם
# machine-drafted, needs native review
transport-next-track = הפריט הבא
# machine-drafted, needs native review
queue-title = תור ההפעלה
# machine-drafted, needs native review
queue-summary = { $count ->
    [one] פריט אחד
    [two] שני פריטים
   *[other] { $count } פריטים
}
# machine-drafted, needs native review
queue-summary-time = { $count ->
    [one] פריט אחד
    [two] שני פריטים
   *[other] { $count } פריטים
} · { $time }
# machine-drafted, needs native review
queue-empty = התור ריק.
    אפשר להוסיף קובצי צליל וסרטונים עם „להפעיל אחר כך” או „הוספה לתור”, או לפתוח רשימת השמעה.
# machine-drafted, needs native review
queue-empty-short = התור ריק.
# machine-drafted, needs native review
queue-playing = מתנגן
# machine-drafted, needs native review
queue-repeat = חזרה
# machine-drafted, needs native review
queue-repeat-off = חזרה: כבויה
# machine-drafted, needs native review
queue-repeat-all = חזרה: על הכול
# machine-drafted, needs native review
queue-repeat-one = חזרה: על פריט אחד
# machine-drafted, needs native review
queue-shuffle = ערבוב
# machine-drafted, needs native review
queue-shuffle-on = ערבוב: פעיל
# machine-drafted, needs native review
queue-shuffle-off = ערבוב: כבוי
# machine-drafted, needs native review
queue-save = שמירה כרשימת השמעה…
# machine-drafted, needs native review
queue-save-tip = שמירת התור כרשימת השמעה מסוג M3U8 או XSPF
# machine-drafted, needs native review
queue-clear = ניקוי…
# machine-drafted, needs native review
queue-clear-tip = ניקוי התור. הקבצים נשארים בדיסק.
# machine-drafted, needs native review
queue-remove = הסרה מהתור (הקובץ נשאר בדיסק)
# machine-drafted, needs native review
queue-network-item = פריט רשת, לא מופעל: { $address }
# machine-drafted, needs native review
queue-artist-title = { $artist } – { $title }
# machine-drafted, needs native review
queue-at-end = אין פריט הבא.
# machine-drafted, needs native review
queue-at-start = אין פריט קודם.
# machine-drafted, needs native review
queue-missing = הקובץ { $name } כבר לא קיים. דילגנו עליו.
# machine-drafted, needs native review
queue-network-not-played = פריטי רשת עדיין לא מופעלים.
# machine-drafted, needs native review
queue-nothing-to-add = אין מה להוסיף. לתור נכנסים רק קובצי צליל, סרטונים ורשימות השמעה.
# machine-drafted, needs native review
queue-added = { $count ->
    [one] פריט אחד נוסף לתור.
    [two] שני פריטים נוספו לתור.
   *[other] { $count } פריטים נוספו לתור.
}
# machine-drafted, needs native review
queue-added-next = { $count ->
    [one] פריט אחד יופעל אחר כך.
    [two] שני פריטים יופעלו אחר כך.
   *[other] { $count } פריטים יופעלו אחר כך.
}
# machine-drafted, needs native review
queue-playlist-opened = ברשימה { $name }: { $count ->
    [one] פריט אחד בתור.
    [two] שני פריטים בתור.
   *[other] { $count } פריטים בתור.
}
# machine-drafted, needs native review
queue-playlist-nothing = שום דבר ברשימה { $name } לא ניתן להפעלה כאן. התור מציג את מה שהיא מכילה.
# machine-drafted, needs native review
queue-playlist-failed = לא ניתן לפתוח את { $name }: { $reason }
# machine-drafted, needs native review
queue-playlist-unreadable = לא ניתן לקרוא את הקובץ.
# machine-drafted, needs native review
queue-playlist-damaged = זו לא רשימת השמעה ש־Specular יודע לקרוא.
# machine-drafted, needs native review
queue-clear-ask = { $count ->
    [one] לנקות את התור (פריט אחד)?
    [two] לנקות את התור (שני פריטים)?
   *[other] לנקות את התור ({ $count } פריטים)?
}
# machine-drafted, needs native review
queue-clear-detail = רק הרשימה מתנקה. הקבצים נשארים בדיסק.
# machine-drafted, needs native review
queue-clear-ok = ניקוי התור
# machine-drafted, needs native review
queue-cleared = התור נוקה.
# machine-drafted, needs native review
queue-save-title = שמירת התור כרשימת השמעה
# machine-drafted, needs native review
queue-default-name = תור
# machine-drafted, needs native review
queue-format-m3u8 = רשימת השמעה M3U8
# machine-drafted, needs native review
queue-format-xspf = רשימת השמעה XSPF
# machine-drafted, needs native review
queue-saved = הקובץ { $name } נשמר.
# machine-drafted, needs native review
queue-save-failed = לא ניתן לשמור את { $name }: { $reason }
# machine-drafted, needs native review
settings-viewing-sound-play-next = הפעלת קובץ הצליל הבא
# machine-drafted, needs native review
settings-viewing-sound-play-next-help = כשקובץ צליל מסתיים, קובץ הצליל הבא בתיקייה מופעל.
# machine-drafted, needs native review
settings-viewing-play-repeat = חזרה
# machine-drafted, needs native review
settings-viewing-play-repeat-help = לקובצי צליל, לסרטונים ולתור.
# machine-drafted, needs native review
settings-viewing-play-repeat-off = כבויה
# machine-drafted, needs native review
settings-viewing-play-repeat-all = על הכול
# machine-drafted, needs native review
settings-viewing-play-repeat-one = על פריט אחד
# machine-drafted, needs native review
settings-viewing-play-shuffle = ערבוב
# machine-drafted, needs native review
settings-viewing-play-shuffle-help = הפעלה בסדר אקראי, כל פריט פעם אחת בכל סבב.
