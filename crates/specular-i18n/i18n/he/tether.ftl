# Specular — tethered capture: a camera on USB, its shots downloaded as they are taken. עברית. machine-drafted, needs native review

palette-tether = צילום מחובר…
palette-tether-capture = צילום תמונה (מחובר)

tether-title = צילום מחובר
tether-close = סגירת הצילום המחובר
tether-default-folder = צילום מחובר

## Cameras
tether-none = לא נמצאה מצלמה.
tether-none-hint = חברו את המצלמה בכבל USB והדליקו אותה. אם יש לה מצב USB, בחרו PTP או חיבור למחשב.
tether-look-again = חיפוש חוזר
tether-connect = חיבור
tether-next-camera = המצלמה הבאה
tether-connecting = מתחבר…
tether-connected = { $camera } מחוברת
tether-disconnect = ניתוק
tether-try-again = ניסיון חוזר
tether-gone = המצלמה נותקה.
tether-not-connected = אין מצלמה מחוברת. פתחו קודם את הצילום המחובר.

## Why a camera can't be opened
tether-permission = למשתמש הזה אין הרשאה לפתוח את התקן ה־USB של המצלמה { $node }.
tether-permission-rule = כלל udev מאפשר זאת. כמנהל מערכת, הוסיפו את השורה הזו לקובץ { $file }:
tether-permission-reload = לאחר מכן הריצו "sudo udevadm control --reload" וחברו את המצלמה מחדש.
tether-copy-rule = העתקת הכלל
tether-busy-other = תוכנה אחרת משתמשת במצלמה.
tether-busy-named = המצלמה בשימוש של { $programs }.
tether-busy-hint = סגרו את התוכנה הזו או נתקו את המצלמה במנהל הקבצים, ונסו שוב.
tether-driver = מנהל ההתקן של המערכת למצלמה הזו לא מאפשר לתוכנות אחרות להשתמש בה.

## The camera
tether-battery = סוללה { PERCENT($percent) }
tether-card = { $card }: { $free } פנויים
tether-card-shots = { $card }: { $free } פנויים, { $shots } צילומים
tether-no-card = אין כרטיס זיכרון.
tether-no-settings = המצלמה לא מדווחת על הגדרות החשיפה שלה.
tether-no-remote = אי אפשר להפעיל את המצלמה הזו מכאן. צילומים שצולמו בכפתור הצמצם שלה יורדו.
tether-capture = צילום
tether-step-down = { $setting }: הורדה ({ $value })
tether-step-up = { $setting }: העלאה ({ $value })

## Exposure settings
tether-prop-mode = מצב
tether-prop-shutter = מהירות תריס
tether-prop-aperture = צמצם
tether-prop-iso = ISO
tether-prop-bias = פיצוי חשיפה
tether-prop-white-balance = איזון לבן
tether-auto = אוטומטי
tether-bulb = חשיפה ארוכה (B)
tether-camera-value = הגדרת מצלמה { $value }
tether-wb-manual = ידני
tether-wb-one-push = מדידה חד־פעמית
tether-wb-daylight = אור יום
tether-wb-fluorescent = פלורסנט
tether-wb-tungsten = טונגסטן
tether-wb-flash = מבזק
tether-mode-manual = ידני
tether-mode-program = תוכנית
tether-mode-aperture = עדיפות צמצם
tether-mode-shutter = עדיפות תריס
tether-mode-creative = יצירתי
tether-mode-action = ספורט
tether-mode-portrait = דיוקן

## Where shots go
tether-folder = תיקייה
tether-names = שמות
tether-names-camera = השמות של המצלמה
tether-arrival = סגנון Develop
tether-choose-folder = תיקייה…
tether-choose-style = סגנון…
tether-downloading = מוריד… { PERCENT($percent) }
tether-saved = { $name } נשמר
tether-saved-count = { $count ->
    [one] צילום אחד נשמר
    [two] שני צילומים נשמרו
   *[other] { $count } צילומים נשמרו
}

## Failures
tether-busy = המצלמה עסוקה. נסו שוב.
tether-card-full = כרטיס הזיכרון מלא.
tether-set-refused = המצלמה לא קיבלה את ההגדרה. בדקו את חוגת המצבים.
tether-disk = לא ניתן לשמור את הצילום: { $error }
tether-error = שגיאת מצלמה: { $error }

## Settings › Cull mode
settings-cull-tether-folder = תיקיית הצילום המחובר
settings-cull-tether-folder-help = לאן נשמרים צילומים ממצלמה המחוברת בכבל.
settings-cull-tether-folder-unset = תמונות › צילום מחובר
settings-cull-tether-rename = שמות הקבצים בצילום מחובר
settings-cull-tether-rename-help = שמות חדשים, עם אותם סימנים כמו בשינוי השם בייבוא. ריק שומר את השמות של המצלמה.
