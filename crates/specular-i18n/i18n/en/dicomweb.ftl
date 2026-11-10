# Specular — DICOMweb servers, study search and streamed series (DESIGN.md §12.8). English.

palette-dicom-servers = DICOMweb servers…

dicomweb-title = DICOMweb servers
dicomweb-servers-intro = Studies on a server are searched and opened only when you ask. Nothing is sent before.
dicomweb-servers = Servers
dicomweb-servers-empty = No servers yet. Add one with its DICOMweb address.
dicomweb-session-only = Passwords and tokens are kept only until Specular closes.
dicomweb-add = Add…
dicomweb-edit = Edit…
dicomweb-remove = Remove
dicomweb-close = Close
dicomweb-browse = Browse
dicomweb-back = Back
dicomweb-save = Save
dicomweb-search = Search
dicomweb-stop = Stop
# $server a server's name.
dicomweb-removed = Removed { $server }
# Remove asks first. $server a server's name.
dicomweb-remove-ask = Remove “{ $server }” from the saved servers?
dicomweb-remove-detail = Nothing on disk is lost. Its password or token for this session is forgotten.
# Shown with servers reached without encryption (http://).
dicomweb-not-encrypted = Not encrypted

# The page that adds or edits a server.
dicomweb-add-title = Add a server
dicomweb-edit-title = Server settings
dicomweb-field-name = Name
dicomweb-hint-name = Optional
dicomweb-field-url = Address
dicomweb-hint-url = https://pacs.example.org/dicom-web
dicomweb-field-auth = Sign-in
dicomweb-auth-none = No sign-in
dicomweb-auth-basic = User name and password
dicomweb-auth-bearer = Access token
dicomweb-field-user = User name
dicomweb-field-password = Password
dicomweb-field-token = Access token
dicomweb-hint-secret-kept = Leave empty to keep the one given
dicomweb-secret-session = The password or token is kept only until Specular closes, never on disk.
dicomweb-secret-needed = Enter the password or token again: it is kept only until Specular closes.
dicomweb-http-warning = This address is not encrypted: the password or token would be sent in the clear.
dicomweb-url-empty = Enter the server’s DICOMweb address.
dicomweb-url-scheme = The address must start with https:// or http://.
dicomweb-url-host = The address has no server name.
dicomweb-url-userinfo = Put the user name and password in the sign-in fields, not in the address.
# $reason why the list could not be saved.
dicomweb-save-failed = The server list could not be saved: { $reason }

# The search page.
dicomweb-search-title = Find studies
dicomweb-field-patient-name = Patient name
dicomweb-field-patient-id = Patient ID
dicomweb-field-date-from = From
dicomweb-field-date-to = To
dicomweb-hint-date = YYYYMMDD
dicomweb-field-modality = Modality
dicomweb-hint-modality = CT, MR…
dicomweb-field-accession = Accession number
dicomweb-show-patient = Show patient data
dicomweb-switch-on = On
dicomweb-switch-off = Off
dicomweb-patient-shown = Patient data shown
dicomweb-patient-hidden = Patient data hidden
dicomweb-studies = Studies
dicomweb-search-hint = Fill in any of the fields and search.
dicomweb-no-studies = No studies match.
dicomweb-searching = Searching…
dicomweb-stopped = Stopped
dicomweb-load-more = Load more
dicomweb-study-untitled = Study without description
dicomweb-series-untitled = Series without description
dicomweb-study-counts = { $series ->
    [one] One series
   *[other] { $series } series
}, { $images ->
    [one] one image
   *[other] { $images } images
}
dicomweb-series-images = { $count ->
    [one] One image
   *[other] { $count } images
}
# $id a patient ID.
dicomweb-patient-id = ID { $id }
# $number an accession number.
dicomweb-accession = Accession { $number }
dicomweb-found = { $count ->
    [one] One study found
   *[other] { $count } studies found
}
dicomweb-series-found = { $count ->
    [one] One series
   *[other] { $count } series
}

# Errors talking to a server. $server a server's name.
dicomweb-error-offline = { $server } cannot be reached. Check the network and the address.
dicomweb-error-auth = The server refused the user name, password or token.
dicomweb-error-not-found = There is no DICOMweb service at this address.
# $reason a technical reason.
dicomweb-error-other = The server could not answer: { $reason }

# A series streamed into the viewer. $series a series' description.
dicomweb-loading = Loading { $series }…
# $done and $total numbers of images.
dicomweb-loading-progress = Loading { $series }: { $done } of { $total }
dicomweb-loaded = { $series }: { $total ->
    [one] one image loaded
   *[other] all { $total } images loaded
}
dicomweb-loaded-failed = { $failed } of { $total } images could not be loaded.
# $server a server's name.
dicomweb-offline-cached = { $server } cannot be reached: showing the images saved earlier.
# $reason a technical reason.
dicomweb-cache-failed = The download folder could not be opened: { $reason }
