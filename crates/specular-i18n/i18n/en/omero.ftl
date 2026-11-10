# Specular — OMERO servers and the Image Data Resource (IDR), browsed read-only. English.

palette-omero-servers = OMERO servers…

omero-title = OMERO servers
omero-servers-intro = Projects, screens and images on a server are listed only when you browse it. Nothing is sent before.
omero-servers = Servers
omero-servers-empty = No servers yet. Add one with its OMERO.web address.
omero-session-only = Passwords are kept only until Specular closes.
omero-add = Add…
omero-edit = Edit…
omero-remove = Remove
omero-close = Close
omero-browse = Browse
omero-back = Back
omero-save = Save
omero-stop = Stop
omero-open = Open
# $server a server's name.
omero-removed = Removed { $server }
# Shown with servers reached without encryption (http://).
omero-not-encrypted = Not encrypted
# A server browsed without signing in.
omero-public = Public
# $user the user name a server is signed in with.
omero-signed-in-as = As { $user }

# The page that adds or edits a server.
omero-add-title = Add a server
omero-edit-title = Server settings
omero-field-name = Name
omero-hint-name = Optional
omero-field-url = Address
omero-hint-url = https://omero.example.org
omero-field-user = User name
omero-hint-user = Empty for public data
omero-field-password = Password
omero-hint-password-kept = Leave empty to keep the one given
omero-password-session = The password is kept only until Specular closes, never on disk.
omero-password-needed = Enter the password: it is kept only until Specular closes.
omero-http-warning = This address is not encrypted: the password would be sent in the clear.
omero-url-invalid = Enter the OMERO.web address, starting with https:// or http://.
# $reason why the list could not be saved.
omero-save-failed = The server list could not be saved: { $reason }

# The browse page.
omero-browse-title = Browse
omero-top = Projects and screens
omero-datasets = Datasets
omero-plates = Plates
omero-wells = Wells
omero-fields = Fields
omero-images = Images
omero-kind-project = Project
omero-kind-dataset = Dataset
omero-kind-image = Image
omero-kind-screen = Screen
omero-kind-plate = Plate
omero-kind-well = Well
# $well a well's row and column, as B7.
omero-well = Well { $well }
omero-well-fields = { $count ->
    [one] One field
   *[other] { $count } fields
}
omero-children = { $count ->
    [one] One item
   *[other] { $count } items
}
omero-load-more = Load more
omero-loading = Loading…
omero-empty = Nothing here
omero-stopped = Stopped
omero-found = { $count ->
    [one] One item
   *[other] { $count } items
}
# $reason why the image could not be opened.
omero-open-failed = The image could not be opened: { $reason }
# The channel keys would hide the last channel shown.
omero-channel-last = At least one channel stays shown
# $server a server's name.
omero-session-ended = The session on { $server } ended. Enter the password to go on.

# Errors.
# $server a server's name.
omero-error-offline = { $server } could not be reached.
omero-error-sign-in = This server needs signing in.
omero-error-expired = The session ended.
omero-error-login = The user name or password was refused.
omero-error-forbidden = Your account may not see this.
omero-error-not-found = Not found on the server (it may have been removed).
# $code an HTTP status code.
omero-error-status = The server answered with error { $code }.
# $reason what was wrong with the answer.
omero-error-malformed = The server’s answer could not be read: { $reason }
