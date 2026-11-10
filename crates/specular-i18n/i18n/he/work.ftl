# machine-drafted, needs native review
# Specular — עבודה ברקע: המחוון שמוצג בזמן שמשהו פועל ברקע, והחלונית שהוא פותח. עברית.

## החלונית

work-title = עבודה ברקע
work-show = הצגת העבודה ברקע
work-empty = שום דבר לא פועל ברקע.

## משימות

work-kind-library = ספריית התמונות
work-kind-ai-search = חיפוש בינה מלאכותית
work-kind-faces = אנשים
work-kind-text = טקסט בתמונות
work-kind-culling = ציוני מיון
work-kind-download = הורדת מודל בינה מלאכותית
work-kind-similar = צילומים דומים

## שלבים

work-step-finding = מחפש תמונות
work-step-reading = קורא את פרטי התמונות
work-step-signatures = משווה תמונות
work-step-grouping = מקבץ צילומים דומים
work-step-text-index = בונה את אינדקס החיפוש
work-step-embedding = מאנדקס תמונות לחיפוש בינה מלאכותית
work-step-switching = מאנדקס מחדש לגודל המודל החדש
work-step-faces = מחפש פנים
work-step-ocr = קורא טקסט בתמונות
work-step-scoring = מדרג חדות וחשיפה
work-step-downloading = מוריד
work-step-focus = בודק חדות
work-step-item = { $step } · { $item }

## התקדמות

work-count-photos = { $done } מתוך { $total } { $count ->
    [one] תמונה
    [two] תמונות
   *[other] תמונות
}
work-count = { $done } מתוך { $total }
work-count-left = { $count } · נותרו { $left }
work-so-far-photos = { $done } { $count ->
    [one] תמונה
    [two] תמונות
   *[other] תמונות
} עד כה
work-so-far = { $done } עד כה

## הזמן שנותר

work-eta-estimating = מעריך…
work-eta-seconds = { $n ->
    [one] נותרה בערך שנייה אחת
    [two] נותרו בערך שתי שניות
   *[other] נותרו בערך { $n } שניות
}
work-eta-minutes = { $n ->
    [one] נותרה בערך דקה אחת
    [two] נותרו בערך שתי דקות
   *[other] נותרו בערך { $n } דקות
}
work-eta-hours = { $h ->
    [one] נותרה בערך שעה אחת
    [two] נותרו בערך שעתיים
   *[other] נותרו בערך { $h } שעות
}
work-eta-hours-minutes = נותרו בערך { $h ->
    [one] שעה אחת
    [two] שעתיים
   *[other] { $h } שעות
} ו־{ $m ->
    [one] דקה אחת
    [two] שתי דקות
   *[other] { $m } דקות
}

## למה משימה ממתינה

work-paused-browsing = מושהה בזמן הדפדוף
work-paused-battery = מושהה בזמן עבודה על סוללה
work-paused-low-power = מושהה במצב חיסכון בחשמל
work-paused-heat = מושהה כל עוד המחשב חם
work-paused-memory = מושהה כל עוד הזיכרון נמוך
work-paused-user = מושהה

## מה בהמשך

work-then = אחר כך: { $steps }

## כפתורים

work-pause = השהיה
work-resume = המשך
work-settings = הגדרות…

## המחוון

work-chip = { $job } · { $done } מתוך { $total }
work-chip-so-far = { $job } · { $done }
work-chip-paused = { $job } · { $reason }
work-chip-more = { $chip } (+{ $more })
work-chip-name = עבודה ברקע: { $status }

## הכרזות

work-started = { $job }: התחיל ברקע
work-finished = { $job }: הסתיים
