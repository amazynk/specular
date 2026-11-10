# Specular — Export slideshow as video (DESIGN.md §6.5). Hebrew. Machine-drafted, needs native review.

palette-export-slideshow = ייצוא המצגת כווידאו…

video-export-title = ייצוא המצגת כווידאו
video-export-resolution = רזולוציה
# $width and $height in pixels.
video-export-resolution-window = חלון ({ $width } × { $height })
video-export-fps = קצב פריימים
# $fps frames per second, a number.
video-export-fps-value = { $fps } fps
video-export-quality = איכות
video-export-quality-high = גבוהה
video-export-quality-standard = רגילה
video-export-quality-compact = דחוסה
video-export-images = { $count ->
    [one] תמונה אחת, לפי הסדר, המעבר והכיתובים של המצגת
    [two] שתי תמונות, לפי הסדר, המעבר והכיתובים של המצגת
   *[other] { $count } תמונות, לפי הסדר, המעבר והכיתובים של המצגת
}
# $length a duration ("2:40"), $size a file size ("38 MB").
video-export-summary = אורך של כ־{ $length }, גודל של כ־{ $size }
video-export-export = ייצוא…
video-export-replace = החלפה
# $file a file name.
video-export-replace-question = { $file } כבר קיים. להחליף אותו?
# $percent a formatted percentage ("42%").
video-export-progress = מייצא… { $percent }
video-export-cancelling = מבטל…
# $file a file name, $size a file size ("38 MB").
video-export-saved = נשמר { $file } ({ $size })
video-export-skipped = { $count ->
    [one] לא ניתן היה לקרוא תמונה אחת והיא הושמטה.
    [two] לא ניתן היה לקרוא שתי תמונות והן הושמטו.
   *[other] לא ניתן היה לקרוא { $count } תמונות והן הושמטו.
}
video-export-failed = לא ניתן לייצא את הווידאו: { $reason }
video-export-cancelled = הייצוא בוטל
video-export-no-images = לא ניתן היה לקרוא אף אחת מהתמונות.
video-export-show-in-folder = הצגה בתיקייה

## Toolbar item and compatibility (machine-drafted, needs native review)
settings-viewing-toolbar-items-export-slideshow = ייצוא מצגת כסרטון
video-export-compatibility = תאימות
video-export-compatibility-current = נגנים עדכניים
video-export-compatibility-all = כל הנגנים
