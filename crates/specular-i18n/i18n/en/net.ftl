# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. English.

## The panel

# The panel's title.
net-title = Downloads
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Show downloads
# The panel when nothing is downloading any more.
net-empty = Nothing is downloading.
# Cancels one download.
net-stop = Stop

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Data from { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Image tiles from { $host }
net-what-point-cloud = Point cloud from { $host }
net-what-map-tiles = Map tiles from { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Map service at { $host }
net-what-dicom = DICOM data from { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = DICOM series { $series } from { $host }
# Studies being sent to a PACS; { $host } is its host and port.
net-what-dicom-send = Sending studies to { $host }
net-what-omero = OMERO data from { $host }
net-what-zarr-chunks = Zarr chunks from { $host }
net-what-zarr-metadata = Zarr metadata from { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Can’t open { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = Model { $name }
net-what-update = Update check

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } of { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } of { $total } { $count ->
    [one] request
   *[other] requests
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
net-images = { $done } of { $total } { $count ->
    [one] image
   *[other] images
}
# Images the server could not send ($n formatted, $count plain).
net-images-failed = { $n } failed
# Images sent with a warning.
net-images-warning = { $n } { $count ->
    [one] warning
   *[other] warnings
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Starting…
# Running, no time left known (many requests, or an unknown size).
net-running = Downloading
net-done = Done
# Done, but $count of its requests failed.
net-done-failed = Done, { $count } failed
# Cancelled with Stop.
net-stopped = Stopped
# $reason is one of the reasons below.
net-failed = Failed: { $reason }
net-reason-offline = no connection
net-reason-timeout = the server did not answer in time
# $code is the HTTP status number.
net-reason-status = the server answered { $code }
net-reason-auth = sign-in refused
net-reason-other = the download broke off

## The indicator

# More downloads than shown in place.
net-more = { $count } more
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Downloads: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Downloading { $what }
net-finished = Downloaded { $what }
net-failed-announce = Download failed: { $what }
net-stopped-announce = Download stopped: { $what }

## No network (Settings › Privacy)

# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = Network is off in Settings › Privacy.
# The panel under the switch while the network is off.
privacy-off-title = The network is off.
privacy-off-what = Update checks, model downloads, DICOM, OMERO and Zarr servers, cloud images and maps are off.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Set by your administrator in { $file }. This switch can’t turn the network back on.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Set for this run by { $what }. This switch can’t turn the network back on.

## Open address… (File)

address-title = Open address
address-intro = A Zarr store, a cloud GeoTIFF (.tif), a COPC point cloud (.copc.laz), a STAC item or catalog (.json) or a DICOMweb server.
address-label = Address (http, https or s3)
address-recent = Recent
address-open = Open
address-network-off = Can’t open addresses: the network is off in Settings › Privacy.
address-problem-empty = Type or paste an address.
address-problem-scheme = The address must start with http://, https:// or s3://.
address-problem-host = The address has no server name, or has spaces in it.
address-problem-kind = This isn’t an address { -app-name } opens: a .zarr store, a .tif, a .copc.laz, a STAC .json or a DICOMweb server.
