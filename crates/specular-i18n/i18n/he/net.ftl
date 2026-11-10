# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. עברית.

## The panel

# The panel's title.
net-title = הורדות
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = הצגת ההורדות
# The panel when nothing is downloading any more.
net-empty = אין הורדות פעילות.
# Cancels one download.
net-stop = עצירה

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = נתונים מ־{ $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = אריחי תמונה מ־{ $host }
net-what-point-cloud = ענן נקודות מ־{ $host }
net-what-map-tiles = אריחי מפה מ־{ $host }
# A map service's capabilities, features or map images.
net-what-map-service = שירות מפות ב־{ $host }
net-what-dicom = נתוני DICOM מ־{ $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = סדרת DICOM { $series } מ־{ $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = שליחת בדיקות אל { $host }
net-what-omero = נתוני OMERO מ־{ $host }
net-what-zarr-chunks = מקטעי Zarr מ־{ $host }
net-what-zarr-metadata = מטא־נתונים של Zarr מ־{ $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = לא ניתן לפתוח את { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = מודל { $name }
net-what-update = בדיקת עדכונים

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } מתוך { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } מתוך { $total } { $count ->
    [one] בקשה
    [two] בקשות
   *[other] בקשות
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } מתוך { $total } { $count ->
    [one] תמונה
    [two] תמונות
   *[other] תמונות
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = נכשלו: { $n }
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] אזהרה
    [two] אזהרות
   *[other] אזהרות
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = מתחיל…
# Running, no time left known (many requests, or an unknown size).
net-running = מוריד
net-done = הסתיים
# Done, but $count of its requests failed.
net-done-failed = הסתיים, נכשלו: { $count }
# Cancelled with Stop.
net-stopped = נעצר
# $reason is one of the reasons below.
net-failed = נכשל: { $reason }
net-reason-offline = אין חיבור
net-reason-timeout = השרת לא ענה בזמן
# $code is the HTTP status number.
net-reason-status = השרת ענה { $code }
net-reason-auth = הכניסה נדחתה
net-reason-other = ההורדה נקטעה

## The indicator

# More downloads than shown in place.
net-more = עוד { $count }
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = הורדות: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = מוריד את { $what }
net-finished = הורד: { $what }
net-failed-announce = ההורדה נכשלה: { $what }
net-stopped-announce = ההורדה נעצרה: { $what }

## ללא רשת (הגדרות › פרטיות)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = הרשת כבויה בהגדרות › פרטיות.
# The panel under the switch while the network is off.
privacy-off-title = הרשת כבויה.
privacy-off-what = בדיקת עדכונים, הורדת מודלים, שרתי DICOM‏, OMERO ו־Zarr, תמונות בענן ומפות כבויים.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = הוגדר על ידי מנהל המערכת ב־{ $file }. מתג זה לא יכול להפעיל את הרשת מחדש.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = הוגדר להרצה הזו על ידי { $what }. מתג זה לא יכול להפעיל את הרשת מחדש.

## Open address… (File)

# machine-drafted, needs native review
address-title = פתיחת כתובת
# machine-drafted, needs native review
address-intro = מאגר Zarr, קובץ GeoTIFF בענן (.tif), ענן נקודות COPC (.copc.laz), פריט או קטלוג STAC (.json) או שרת DICOMweb.
address-label = כתובת (http, https או s3)
address-recent = אחרונות
address-open = פתיחה
address-network-off = לא ניתן לפתוח כתובות: הרשת כבויה בהגדרות › פרטיות.
address-problem-empty = יש להקליד או להדביק כתובת.
address-problem-scheme = הכתובת חייבת להתחיל ב-http://, ב-https:// או ב-s3://.
address-problem-host = אין בכתובת שם שרת, או שיש בה רווחים.
# machine-drafted, needs native review
address-problem-kind = זו לא כתובת ש-{ -app-name } פותח: מאגר .zarr, קובץ .tif, קובץ .copc.laz, קובץ STAC‏ .json או שרת DICOMweb.
