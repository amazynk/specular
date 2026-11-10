# DICOM series and the tag browser. machine-drafted, needs native review

palette-dicom-tags = הצגת תגי DICOM
palette-next-series = הסדרה הבאה
palette-prev-series = הסדרה הקודמת
disabled-not-dicom = התמונה אינה קובץ DICOM
disabled-no-other-series = אין סדרה אחרת בתיקייה זו
dicom-tags-title = תגי DICOM
dicom-tags-title-file = תגי DICOM: { $file }
dicom-tags-filter = סינון תגים
dicom-tags-show-patient = הצגת נתוני מטופל
dicom-tags-col-tag = תג
dicom-tags-col-keyword = מילת מפתח
dicom-tags-col-vr = VR
dicom-tags-col-length = אורך
dicom-tags-col-value = ערך
dicom-tags-hidden = מוסתר
dicom-tags-hidden-count = ערכי מטופל מוסתרים: { $count }
dicom-tags-no-match = אין תגים תואמים
dicom-tags-reading = קורא תגים…
dicom-tags-error = לא ניתן לקרוא את התגים: { $reason }
dicom-tags-items = פריטים: { $count }
dicom-tags-fragments = מקטעים: { $count }
dicom-tags-item = פריט { $index }
dicom-tags-bytes = בתים: { $count }
dicom-tags-empty = (ריק)
dicom-tags-omitted = רכיבים נוספים שלא פורטו: { $count }
dicom-series-group = סדרה
dicom-series-slice = פרוסה
dicom-series-slice-value = { $index } מתוך { $count }
dicom-series-frames = פריימים: { $frames }
dicom-series-location = מיקום הפרוסה
dicom-series-mm = { $mm } מ״מ
dicom-series-spacing = מרווח בין פרוסות
dicom-series-which = סדרה
dicom-series-which-value = { $index } מתוך { $count }
dicom-series-order = סדר
dicom-series-order-position = לפי מיקום
dicom-series-order-instance = לפי מספר מופע
dicom-series-order-time = לפי זמן רכישה
dicom-series-order-name = לפי שם קובץ
dicom-series-gaps = פרוסות חסרות
dicom-series-gaps-value = פרוסות חסרות: { $count }
dicom-series-gaps-where = אחרי פרוסה { $slices }
dicom-series-irregular = מרווח לא סדיר
dicom-series-tilt = הטיית גנטרי
dicom-series-tilt-value = { $degrees }°
dicom-series-duplicates = כפילויות דולגו
dicom-series-toast = { $series ->
    [one] סדרה – פרוסות: { $count }
    [two] סדרה { $index } מתוך { $series } – פרוסות: { $count }
   *[other] סדרה { $index } מתוך { $series } – פרוסות: { $count }
}
dicom-series-capped = נקראו רק הקבצים הראשונים בתיקייה: { $count }

studies-title = בדיקות
studies-title-medium = בדיקות: { $medium }
studies-show-patient = הצגת נתוני המטופל
studies-patient-hidden = מוסתר
studies-patient-hidden-note = שמות המטופלים ותאריכי הלידה מוסתרים
studies-none = לא נמצאו בדיקות
studies-reading = קורא את הדיסק…
studies-reading-count = קורא את הדיסק: { $done } מתוך { $total ->
        [one] קובץ אחד
        [two] שני קבצים
       *[other] { $total } קבצים
    }
studies-opening = פותח את הסדרה…
studies-error = לא ניתן לקרוא את הדיסק: { $reason }
studies-missing = קבצים הרשומים בדיסק חסרים: { $count }
studies-capped = נקראו רק הקבצים הראשונים: { $count }
# machine-drafted, needs native review
studies-stopped = הקריאה נעצרה אחרי { $done } מתוך { $total ->
        [one] קובץ אחד
        [two] שני קבצים
       *[other] { $total } קבצים
    }: הרשימה חלקית
studies-stopped-early = הקריאה נעצרה: הרשימה חלקית
studies-cancel-description = עוצר את קריאת הדיסק. מה שנקרא עד כה נשאר ברשימה.
studies-patient-n = מטופל { $index }
studies-patient-unknown = מטופל לא ידוע
studies-born = תאריך לידה: { $date }
studies-no-description = אין תיאור
studies-series = סדרה { $number }: { $description }
studies-series-number = סדרה { $number }
studies-images = תמונות: { $count }
studies-images-missing = חסרות: { $count }
studies-others = אובייקטים אחרים: { $count }
studies-no-images = לסדרה זו אין תמונות בדיסק
studies-medium-inserted = הוכנס דיסק עם בדיקות רפואיות
studies-medium-show = הצגת בדיקות
palette-studies = הצגת הבדיקות בדיסק זה
