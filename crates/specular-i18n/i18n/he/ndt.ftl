# machine-drafted, needs native review
# Inspection probes in the measurement tool (NDT): radiograph evaluation and crack widths.

ndt-tool = בדיקה
ndt-probe = כלי בדיקה: { $name }
ndt-probe-profile = פרופיל לאורך קו
ndt-probe-snr = SNR
ndt-probe-duplex-iqi = מחוון IQI דו־תילי
ndt-probe-wire-iqi = מחוון IQI חד־תילי
ndt-probe-crack = רוחב סדק
ndt-probe-density = נקודת צפיפות

ndt-hint-profile = יש לגרור קו כדי להציג את ערכי האפור לאורכו. [ ו־] מחליפים את כלי הבדיקה.
ndt-hint-snr = יש לגרור אזור בשטח אחיד. הוא מוגדל לפחות ל־20 × 55 פיקסלים.
ndt-hint-duplex-iqi = יש לגרור קו על פני כל זוגות התילים, לאורך מחוון ה־IQI‏.
ndt-hint-wire-iqi = יש לגרור קו קצר לרוחב התיל הדק ביותר שנראה.
ndt-hint-crack = יש ללחוץ לאורך הסדק ואז על Enter‏. הרוחב נקרא ב־10 נקודות.
ndt-hint-density = יש ללחוץ על נקודה כדי לקרוא את ערך האפור שלה. הקלדת הצפיפות הידועה שלה מכיילת.

ndt-refused-needs-scale = יש לקבוע קודם קנה מידה: גודלי התילים נמדדים במילימטרים. יש להשתמש בכיול על אורך ידוע.
ndt-refused-nothing-found = שום דבר לא בולט מעל הרעש לאורך הקו הזה.
ndt-refused-too-small = התמונה קטנה מאזור ה־SNR‏ בגודל 20 × 55 פיקסלים.
ndt-refused-no-noise = באזור הזה אין רעש למדידה (כל הפיקסלים זהים).

ndt-reading = קורא…
ndt-no-pixels = לא ניתן לקרוא את הפיקסלים.

ndt-profile-range = אפור { $min } עד { $max }
ndt-profile-mean = אפור ממוצע { $mean }
ndt-profile-density = צפיפות { $min } עד { $max }

ndt-snr = SNR { $value }
ndt-snrn = SNRn { $value } (SRb { $srb } µm)
ndt-snrn-needs-srb = ‏SNRn דורש את ה־SRb‏: יש למדוד מחוון IQI דו־תילי או להקליד את הערך.
ndt-snr-stats = ממוצע { $mean }, σ { $sd }, אזור { $w } × { $h } px

ndt-duplex-result = הזוג הראשון שאינו מופרד { $pair }: SRb { $srb } µm, אי־חדות { $unsharpness }
ndt-duplex-all-resolved = כל הזוגות שנמצאו מופרדים.
ndt-duplex-dips = שקעים: { $dips }
# machine-drafted, needs native review
# Interpolated values at the 20 % dip (ISO 19232-5:2018).
ndt-duplex-isrb = ערכים מאונטרפלים: iSRb { $isrb } µm, אי־חדות { $unsharpness }

ndt-wire-result = תיל { $wire }, { $diameter }
ndt-wire-measured = רוחב נמדד { $width } (FWHM), קוטר של כ־{ $diameter }

ndt-crack-mean = רוחב ממוצע { $width }
ndt-crack-range = הצר ביותר { $min }, הרחב ביותר { $max }
ndt-crack-cuts = נקודות מדידה: { $count }

ndt-density-value = אפור { $grey }, צפיפות { $density }
ndt-density-grey = אפור { $grey }
ndt-density-uncalibrated = הצפיפות עדיין לא כוילה: יש להקליד צפיפויות ידועות בשתי נקודות.

ndt-ask-density = צפיפות ידועה כאן: { $value } (Enter מחיל, Enter ריק מדלג)
ndt-ask-srb = רזולוציה מרחבית בסיסית SRb: { $value } µm (Enter מחיל, Esc מדלג)
ndt-bad-density = יש להקליד צפיפות בין 0 ל־10.
ndt-bad-srb = יש להקליד את ה־SRb במיקרומטרים, למשל 80.
ndt-srb-set = ה־SRb נקבע ל־{ $srb } µm עבור התמונה הזו.
ndt-density-one-more = נקודת הצפיפות נשמרה. יש להקליד את הצפיפות בנקודה אחת נוספת.
ndt-density-calibrated = הצפיפות כוילה משתי נקודות.
ndt-sidecar-failed = לא ניתן לשמור את הגדרות הבדיקה לצד הקובץ: { $reason }

# Drone photos on the gallery's map, and KML/KMZ export.
map-height-above-takeoff = { $metres } מ׳ מעל נקודת ההמראה
map-export-kml = שמירת התמונות כ־KMZ או KML‏…
map-export-kmz-full = שמירת התמונות כ־KMZ‏ עם התמונות המלאות…
map-kml-heading = כיוון { $degrees }°
map-kml-height = { $metres } מ׳ מעל נקודת ההמראה
map-kml-nothing = אין תמונות עם מיקום לשמירה.
map-kml-saved = תמונות שנשמרו ב־{ $name }: { $count }.
map-kml-failed = לא ניתן לשמור את קובץ המפה: { $reason }

# The inspection report (any photo), and the report's extra outputs.
inspection-report-heading = דוח בדיקה
inspection-report-default-title = דוח בדיקה
inspection-report-folder = הוספת שאר התמונות בתיקייה הזו
inspection-report-docx = שמירה גם כמסמך Word‏ (DOCX)
inspection-report-kmz = שמירה גם של KMZ‏ עם מקומות הצילום
inspection-report-caption = התמונה עם המדידות
inspection-report-col-value = ערך
inspection-report-col-scale = קנה מידה לפי
inspection-report-docx-failed = לא ניתן לכתוב את מסמך ה־Word‏.
inspection-report-unreadable = לא ניתן לקרוא אף אחת מהתמונות.
