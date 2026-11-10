Specular for Windows (portable)
===============================

Run specular.exe. Nothing needs installing.

This copy is portable: settings, caches, the Library's catalogue and logs are kept in this
folder, so it can move or live on a USB stick. The file named "portable" turns that on;
delete it to keep them in your Windows profile instead, as an installed copy does.

Some files (HEIC images Specular cannot decode itself, RAR and CBR archives) are read by
specular-decode-host.exe, which runs in a Windows sandbox (an AppContainer) so that a
malicious file cannot reach the rest of your computer. Keep it next to specular.exe; it
needs no setup.
