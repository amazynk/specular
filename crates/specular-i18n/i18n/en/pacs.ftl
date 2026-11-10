## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Kind
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Host
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Port
dicomweb-hint-port = 104 or 11112
# The application entity title the server answers to.
dicomweb-field-ae = Server AE title
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Our AE title
dicomweb-field-tls = Secure connection (TLS)
dicomweb-tls-on = Secure connection on
dicomweb-tls-off = Secure connection off
dicomweb-problem-host = Enter the server's host name or address.
dicomweb-problem-port = The port is a number from 1 to 65535.
dicomweb-problem-ae = An AE title has 1 to 16 characters, without a backslash.
dicomweb-dimse-note-open = DICOM (DIMSE) servers can be searched, opened and sent to.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
pacs-field-retrieve = Retrieve with
pacs-retrieve-get = C-GET
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
pacs-field-receive-address = Receive on address
pacs-hint-receive-address = This computer's address, like 192.168.1.20
pacs-field-receive-port = Receive on port
pacs-hint-receive-port = Like 11113
pacs-move-note = For C-MOVE the server must know our AE title { $ae } with this address and port.
    Specular listens there only while a series comes in.
pacs-problem-receive-address = Enter this computer's address (like 192.168.1.20) to receive on.
pacs-problem-receive-port = Enter a port to receive on. C-MOVE needs one.
dicomweb-verify = Verify
dicomweb-verifying = Checking the server…
dicomweb-verify-ok = { $server } answered.

## The Studies browser and the palette.

studies-send = Send to PACS…
studies-send-description = Sends this series, its study or everything on the disc to a server.
palette-send-to-pacs = Send to PACS…
palette-send-to-pacs-help = Send the current series or study to a PACS server.

## The Send page.

pacs-title = Send to PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] 1 study
   *[other] { $studies } studies
} · { $files ->
    [one] 1 file
   *[other] { $files } files
}
pacs-what = Send
pacs-scope-series = This series ({ $files ->
    [one] 1 file
   *[other] { $files } files
})
pacs-scope-study = This study ({ $files ->
    [one] 1 file
   *[other] { $files } files
})
pacs-scope-all = Everything read ({ $studies ->
    [one] 1 study
   *[other] { $studies } studies
}, { $files ->
    [one] 1 file
   *[other] { $files } files
})
pacs-anonymise = Anonymise
pacs-anonymise-note = Patient details are removed from the copies sent. Your files are not changed.
pacs-identified-note = The files are sent as they are, with patient details.
# The list of servers to choose from.
pacs-to = To
pacs-no-servers = No servers yet. Add one in DICOMweb servers.
pacs-send = Send…
pacs-go-back = Go back
pacs-confirm-send = Send
pacs-confirm = Send { $studies ->
    [one] 1 study
   *[other] { $studies } studies
} ({ $files ->
    [one] 1 file
   *[other] { $files } files
}) to { $server }?
pacs-busy = A send is still running.
pacs-started = Sending.
pacs-stopping = Stopping…
pacs-sending = Sending { $done } of { $total } to { $server }…
pacs-sent-all = All { $total ->
    [one] 1 file was
   *[other] { $total } files were
} sent to { $server }.
pacs-sent-some = { $sent } of { $total } files were sent to { $server }.
pacs-stopped = Stopped after { $sent } of { $total ->
        [one] { $total } file
       *[other] { $total } files
    }.
pacs-stopped-because = Stopped after { $sent } of { $total ->
        [one] { $total } file
       *[other] { $total } files
    }: { $reason }
pacs-warnings = { $count ->
    [one] 1 file was stored with a warning.
   *[other] { $count } files were stored with a warning.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = And { $count } more.

## Why one file was not sent.

pacs-reason-unreadable = Could not be read: { $reason }
pacs-reason-not-dicom = Not a DICOM file
pacs-reason-burned-in = Has patient details burned into the image, so it is not sent anonymised
pacs-reason-document = A document that cannot be anonymised
pacs-reason-anonymise = Could not be anonymised: { $reason }
pacs-reason-not-accepted = The server does not take { $class }
pacs-reason-convert = Could not be converted for the server: { $reason }
pacs-reason-resources = The server is out of space or resources
pacs-reason-mismatch = The server says the data does not match its type
pacs-reason-understand = The server could not read it
pacs-reason-class = The server does not support this kind of object
pacs-reason-not-authorised = The server does not allow it
pacs-reason-status = The server refused it (status { $code })
pacs-reason-refused = The server refused it
pacs-reason-too-large = Too large for the server
pacs-reason-http = The server refused it (error { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = The network is switched off.
pacs-fatal-unknown-host = { $host } is not a known host name.
pacs-fatal-connect = Nothing answers at { $host }, port { $port }.
pacs-fatal-unreachable = { $host } cannot be reached.
pacs-fatal-timeout = { $host } did not answer in time.
pacs-fatal-cancelled = Stopped.
pacs-fatal-called-ae = { $host } does not answer to the AE title { $ae }. Check the server's AE title.
pacs-fatal-calling-ae = { $host } does not know our AE title { $ae }. Add it on the server.
pacs-fatal-busy = { $host } is busy. Try again later.
pacs-fatal-rejected = { $host } refused the connection.
pacs-fatal-aborted = { $host } ended the connection.
pacs-fatal-nothing-accepted = { $host } takes none of these kinds of objects.
pacs-fatal-protocol = { $host } answered in a way Specular does not understand.
pacs-fatal-auth = { $host } did not accept the sign-in.
pacs-fatal-http = { $host } answered with error { $status }.

## Teaching quiz.

palette-teaching-quiz = Teaching quiz
palette-teaching-quiz-help = Step through the key images or a teaching case, the findings hidden until you reveal them.
# "Case 2 of 5".
quiz-case = Case { $n } of { $total }
quiz-hint-hidden = Space reveals the findings · → next case · Esc ends
quiz-hint-revealed = Space hides them again · → next case · Esc ends
quiz-hidden = Findings hidden.
quiz-no-findings = No findings recorded.
quiz-notes = Case notes: { $notes }
quiz-ended = Quiz ended.
quiz-empty = There are no cases to show. Mark key images with K, or choose a teaching case folder.
quiz-not-a-case = This folder is not a teaching case exported by Specular.
quiz-pick-title = Choose a teaching case folder

## Teaching case folder (Export key images).

figure-keys-case = Teaching case folder
figure-case-notes = Case notes
figure-case-findings = Findings
figure-case-none = None recorded
figure-case-readme = Teaching case exported by Specular. Patient details are removed.
    images: the key images, burned-in text blacked out.
    DICOM: anonymised copies and a Key Object Selection manifest "For Teaching".
    case.txt: the case notes and each image's findings. case.json: the same for scripts.
    For Radiopaedia: upload the DICOM folder or the images, and copy the notes from case.txt.

## Search on a DIMSE server (C-FIND).
pacs-find-not-supported = { $host } does not answer searches (C-FIND).
pacs-find-status = { $host } refused the search (status { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
pacs-retrieve-no-get = { $host } does not send series with C-GET.
    Choose C-MOVE in the server's settings.
pacs-retrieve-no-move = { $host } does not take C-MOVE requests.
pacs-retrieve-unknown-destination = { $host } does not know our AE title { $ae }.
    Add it on the server with this computer's address and port { $port }.
pacs-retrieve-resources = { $host } is out of resources. Try again later.
pacs-retrieve-status = { $host } refused to send the series (status { $code }).
pacs-retrieve-no-matches = { $host } has no images for this series.
pacs-retrieve-roles = { $host } refused to send images over this connection.
    Choose C-MOVE in the server's settings.
pacs-retrieve-move-nothing = { $host } could not send the images to this computer.
    Check the address, the port and the firewall, and that the server knows our AE title { $ae }.
pacs-retrieve-nothing = { $host } could not send { $count ->
    [one] the image
   *[other] any of the { $count } images
} in a form Specular reads.
pacs-retrieve-listen = Cannot receive on { $address }, port { $port }: { $reason }

## DICOM over TLS.
pacs-fatal-tls-certificate = The certificate of { $host } is not trusted.
    Its issuer must be in the system's trusted certificates.
pacs-fatal-tls = No secure connection to { $host }.
    Check that port { $port } speaks DICOM over TLS.
