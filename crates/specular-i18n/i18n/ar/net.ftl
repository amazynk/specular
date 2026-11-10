# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. العربية.

## The panel

# The panel's title.
net-title = التنزيلات
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = إظهار التنزيلات
# The panel when nothing is downloading any more.
net-empty = لا يجري تنزيل أي شيء.
# Cancels one download.
net-stop = إيقاف

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = بيانات من { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = بلاطات صورة من { $host }
net-what-point-cloud = سحابة نقطية من { $host }
net-what-map-tiles = بلاطات خريطة من { $host }
# A map service's capabilities, features or map images.
net-what-map-service = خدمة خرائط على { $host }
net-what-dicom = بيانات DICOM من { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = سلسلة DICOM { $series } من { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = إرسال دراسات إلى { $host }
net-what-omero = بيانات OMERO من { $host }
net-what-zarr-chunks = أجزاء Zarr من { $host }
net-what-zarr-metadata = بيانات Zarr الوصفية من { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = تعذّر فتح { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = النموذج { $name }
net-what-update = التحقق من التحديثات

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } من { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } من { $total } { $count ->
    [zero] طلب
    [one] طلب
    [two] طلبين
    [few] طلبات
    [many] طلبًا
   *[other] طلب
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } من { $total } { $count ->
    [zero] صورة
    [one] صورة
    [two] صورتين
    [few] صور
    [many] صورة
   *[other] صورة
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = فشل: { $n }
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [zero] تحذير
    [one] تحذير
    [two] تحذيران
    [few] تحذيرات
    [many] تحذيرًا
   *[other] تحذير
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = جارٍ البدء…
# Running, no time left known (many requests, or an unknown size).
net-running = جارٍ التنزيل
net-done = اكتمل
# Done, but $count of its requests failed.
net-done-failed = اكتمل، وفشل منها: { $count }
# Cancelled with Stop.
net-stopped = أُوقف
# $reason is one of the reasons below.
net-failed = فشل: { $reason }
net-reason-offline = لا يوجد اتصال
net-reason-timeout = لم يُجب الخادم في الوقت المحدد
# $code is the HTTP status number.
net-reason-status = أجاب الخادم بـ { $code }
net-reason-auth = رُفض تسجيل الدخول
net-reason-other = انقطع التنزيل

## The indicator

# More downloads than shown in place.
net-more = { $count } أخرى
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = التنزيلات: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = جارٍ تنزيل { $what }
net-finished = اكتمل تنزيل { $what }
net-failed-announce = فشل التنزيل: { $what }
net-stopped-announce = أُوقف التنزيل: { $what }

## بلا شبكة (الإعدادات › الخصوصية)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = الشبكة مُطفأة في الإعدادات › الخصوصية.
# The panel under the switch while the network is off.
privacy-off-title = الشبكة مُطفأة.
privacy-off-what = التحقق من التحديثات وتنزيل النماذج وخوادم DICOM وOMERO وZarr والصور السحابية والخرائط كلها مُطفأة.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = ضبطه المسؤول في { $file }. لا يمكن لهذا المفتاح إعادة تشغيل الشبكة.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = ضُبط لهذا التشغيل بواسطة { $what }. لا يمكن لهذا المفتاح إعادة تشغيل الشبكة.

## Open address… (File)

# machine-drafted, needs native review
address-title = فتح عنوان
# machine-drafted, needs native review
address-intro = مخزن Zarr، أو GeoTIFF سحابي (.tif)، أو سحابة نقطية COPC ‏(.copc.laz)، أو عنصر أو فهرس STAC ‏(.json)، أو خادم DICOMweb.
address-label = العنوان (http أو https أو s3)
address-recent = الأخيرة
address-open = فتح
address-network-off = لا يمكن فتح العناوين: الشبكة مُطفأة في الإعدادات › الخصوصية.
address-problem-empty = اكتب عنوانًا أو الصقه.
address-problem-scheme = يجب أن يبدأ العنوان بـ http:// أو https:// أو s3://.
address-problem-host = لا يحتوي العنوان على اسم خادم، أو فيه مسافات.
# machine-drafted, needs native review
address-problem-kind = ليس عنوانًا يفتحه { -app-name }: مخزن ‎.zarr، أو ملف ‎.tif، أو ‎.copc.laz، أو ملف STAC ‎.json، أو خادم DICOMweb.
