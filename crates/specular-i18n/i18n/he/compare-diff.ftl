# Compare: split, signed difference, flicker, alignment, metrics and the pixel probe
# (DESIGN.md §6.3).

## Actions (palette, cheat sheet, remote control)
palette-compare-with = השוואה עם…
palette-compare-with-help = השוואת התמונה הזו לתמונה אחרת לבחירה.
palette-compare-side = השוואה זו לצד זו
palette-compare-side-help = הצגת התמונות המושוות זו לצד זו.
palette-compare-split = השוואה עם קו הפרדה
palette-compare-split-help = הצגת התמונה השנייה מעל הראשונה, חתוכה בקו הפרדה שאפשר לגרור.
palette-compare-split-direction = סיבוב קו ההפרדה
palette-compare-split-direction-help = הצבת קו ההפרדה לרוחב במקום לאורך, או בחזרה.
palette-compare-overlay = השוואה כשכבת־על
palette-compare-overlay-help = הצגת התמונה השנייה מעל הראשונה, במיזוג.
palette-compare-difference = השוואת הפרש
palette-compare-difference-help = הצגת המקומות שבהם התמונות שונות; שוב לסוג הבא.
palette-compare-flicker = הבהוב בין התמונות המושוות
palette-compare-flicker-help = הצגת התמונה האחרת מיד.
palette-compare-auto-flicker = הבהוב אוטומטי
palette-compare-auto-flicker-help = מעבר אוטומטי בין שתי התמונות, או עצירה.
palette-compare-align = יישור התמונות המושוות
palette-compare-align-help = יישור תמונות בגדלים שונים: לפינה העליונה השמאלית, למרכז, בשינוי קנה מידה או באיתור אוטומטי.
palette-compare-metric-space = החלפת מרחב המדידה
palette-compare-metric-space-help = מדידה באור ליניארי או ב־sRGB מקודד.
palette-compare-whole-image = מדידת התמונה כולה
palette-compare-whole-image-help = מדידת התמונה כולה במקום החלק הנראה, או בחזרה.
palette-compare-probe = גשש פיקסלים בהשוואה
palette-compare-probe-help = הצגת הערכים של שתי התמונות מתחת לסמן.
palette-compare-link = קישור או ניתוק התמונות המושוות
palette-compare-link-help = זום והזזה של כל אחת משתי התמונות בנפרד, או שוב יחד.
palette-compare-close = סגירת ההשוואה
palette-compare-close-help = חזרה לגלריה.
disabled-compare-closed = יש לפתוח קודם את ההשוואה
disabled-compare-not-two = פועל עם שתי תמונות

## Cheat sheet
cheatsheet-compare-split = קו הפרדה או זו לצד זו
cheatsheet-compare-split-direction = קו הפרדה לרוחב או לאורך
cheatsheet-compare-auto-flicker = הבהוב אוטומטי
cheatsheet-compare-align = היישור הבא
cheatsheet-compare-space = מרחב מדידה
cheatsheet-compare-whole = התמונה כולה או החלק הנראה
cheatsheet-compare-probe = גשש פיקסלים
cheatsheet-compare-link = קישור או ניתוק שתי תמונות

## Settings
settings-gallery-compare-metric-space = מרחב המדידה בהשוואה
settings-gallery-compare-metric-space-help = היכן ההשוואה מודדת את MAE, MSE, PSNR ו־SSIM. אור ליניארי הוא מה שהמסך מציג. sRGB תואם כלים אחרים בקובצי 8-bit.
settings-gallery-compare-metric-space-linear = אור ליניארי
settings-gallery-compare-metric-space-srgb = sRGB מקודד
settings-gallery-compare-flicker-limit = הבהוב אוטומטי מהיר ביותר
settings-gallery-compare-flicker-limit-help = לכל היותר החלפות תמונה בשנייה. מעל 3, הבהוב עלול לעורר התקפים.

## Toolbar and captions
compare-split = פיצול
compare-split-down = קו הפרדה לאורך
compare-split-across = קו הפרדה לרוחב
compare-link = מקושרות
compare-auto-flicker = אוטומטי
compare-flicker-rate = { $rate } החלפות בשנייה
compare-gain = הגבר ×{ $gain }
compare-probe = גשש
compare-kind-signed = שגיאה עם סימן
compare-kind-on-image = שגיאה מעל התמונה
compare-align-top-left = פינה עליונה שמאלית
compare-align-centre = ממורכז
compare-align-scaled = B בקנה מידה של A
compare-align-auto = יישור אוטומטי
compare-space-linear = אור ליניארי
compare-space-srgb = sRGB
compare-space-help-linear = נמדד באור ליניארי, כפי שמוצג אחרי מיפוי גוונים. PSNR כאן אינו ה־PSNR הרגיל של sRGB ב־8-bit.
compare-space-help-srgb = נמדד על ערכי sRGB מקודדים, כפי שכלים אחרים עושים בקובצי 8-bit. HDR מעל הלבן של SDR נחשב לבן.
compare-space-help-common = ΔE2000 הוא תמיד CIELAB (D65), הלבן של SDR הוא L* 100, ו־HDR נחתך. תמונות גדולות מ־{ $size } פיקסלים נמדדות מעותק של { $size } פיקסלים.
compare-region-view = החלק הנראה
compare-region-whole = התמונה כולה
compare-ssim = SSIM { $ssim }
compare-ssim-small = SSIM דורש 11 פיקסלים
compare-delta-e = ΔE { $mean } ({ PERCENT(95) }: { $p95 }, מקסימום { $max })
compare-delta-e-hdr = HDR נחתך עבור ΔE
compare-aligned-centre = ממורכז, הושוו { $width } × { $height }
compare-aligned-scaled = B בקנה מידה { $scale }×, הושוו { $width } × { $height }
compare-aligned-auto = יושר { $angle }° { $scale }×, הושוו { $width } × { $height }
compare-aligning = מיישר…
compare-align-failed = לא ניתן ליישר את התמונות האלה. חזרה לפינה העליונה השמאלית.
compare-split-label = { $b } מעל { $a }, קו ההפרדה ב־{ PERCENT($percent) }
compare-showing-which = מוצג { $which }: { $name }
compare-auto-flicker-on = מהבהב, { $rate } החלפות בשנייה
compare-auto-flicker-off = ההבהוב נעצר
compare-linked = התמונות מקושרות
compare-unlinked = הקישור בין התמונות נותק: כל אחת זזה בנפרד
compare-with-title = השוואה עם
compare-probe-delta-e = ΔE2000
compare-probe-a11y = A: { $a }. B: { $b }. ΔE2000 { $delta }.
compare-a11y-divider = קו הפרדה
compare-a11y-space = מרחב מדידה
compare-a11y-align = יישור
