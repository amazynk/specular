# machine-drafted, needs native review

## Send to PACS (C-STORE, STOW-RS), DIMSE servers, teaching quiz and teaching case export.

## Servers: DIMSE fields and Verify.

dicomweb-field-kind = Tipo
dicomweb-kind-web = DICOMweb
dicomweb-kind-dimse = DICOM (DIMSE)
dicomweb-field-host = Host
dicomweb-hint-host = pacs.example.org
dicomweb-field-port = Puerto
dicomweb-hint-port = 104 o 11112
# The application entity title the server answers to.
dicomweb-field-ae = AE title del servidor
dicomweb-hint-ae = PACS
# The application entity title Specular sends as.
dicomweb-field-our-ae = Nuestro AE title
# machine-drafted, needs native review
dicomweb-field-tls = Conexión segura (TLS)
dicomweb-tls-on = Conexión segura activada
dicomweb-tls-off = Conexión segura desactivada
dicomweb-problem-host = Escriba el nombre de host o la dirección del servidor.
dicomweb-problem-port = El puerto es un número de 1 a 65535.
dicomweb-problem-ae = Un AE title tiene de 1 a 16 caracteres, sin barra invertida.
# machine-drafted, needs native review
dicomweb-dimse-note-open = Los servidores DICOM (DIMSE) admiten búsquedas, apertura de series y envíos.
# How series are fetched: C-GET (over our own connection) or C-MOVE (the server connects back).
# machine-drafted, needs native review
pacs-field-retrieve = Recuperar con
# machine-drafted, needs native review
pacs-retrieve-get = C-GET
# machine-drafted, needs native review
pacs-retrieve-move = C-MOVE
# C-MOVE: the address of this computer and the port the server sends to.
# machine-drafted, needs native review
pacs-field-receive-address = Recibir en la dirección
# machine-drafted, needs native review
pacs-hint-receive-address = Dirección de este equipo, p. ej. 192.168.1.20
# machine-drafted, needs native review
pacs-field-receive-port = Recibir en el puerto
# machine-drafted, needs native review
pacs-hint-receive-port = P. ej. 11113
# machine-drafted, needs native review
pacs-move-note = Para C-MOVE, el servidor debe conocer nuestro AE title { $ae } con esta dirección y este puerto.
    Specular solo escucha ahí mientras llega una serie.
# machine-drafted, needs native review
pacs-problem-receive-address = Escriba la dirección de este equipo (p. ej. 192.168.1.20) en la que recibir.
# machine-drafted, needs native review
pacs-problem-receive-port = Escriba un puerto en el que recibir. C-MOVE lo necesita.
dicomweb-verify = Verificar
dicomweb-verifying = Comprobando el servidor…
dicomweb-verify-ok = { $server } ha respondido.

## The Studies browser and the palette.

studies-send = Enviar a PACS…
studies-send-description = Envía esta serie, su estudio o todo lo que hay en el disco a un servidor.
palette-send-to-pacs = Enviar a PACS…
palette-send-to-pacs-help = Enviar la serie o el estudio actual a un servidor PACS.

## The Send page.

pacs-title = Enviar a PACS
# Under the title: what is being sent.
pacs-summary = { $studies ->
    [one] { $studies } estudio
    [many] { $studies } estudios
   *[other] { $studies } estudios
} · { $files ->
    [one] { $files } archivo
    [many] { $files } archivos
   *[other] { $files } archivos
}
pacs-what = Enviar
pacs-scope-series = Esta serie ({ $files ->
    [one] { $files } archivo
    [many] { $files } archivos
   *[other] { $files } archivos
})
pacs-scope-study = Este estudio ({ $files ->
    [one] { $files } archivo
    [many] { $files } archivos
   *[other] { $files } archivos
})
pacs-scope-all = Todo lo leído ({ $studies ->
    [one] { $studies } estudio
    [many] { $studies } estudios
   *[other] { $studies } estudios
}, { $files ->
    [one] { $files } archivo
    [many] { $files } archivos
   *[other] { $files } archivos
})
pacs-anonymise = Anonimizar
pacs-anonymise-note = Los datos del paciente se eliminan de las copias enviadas. Sus archivos no cambian.
pacs-identified-note = Los archivos se envían tal cual, con los datos del paciente.
# The list of servers to choose from.
pacs-to = A
pacs-no-servers = Aún no hay servidores. Añada uno en Servidores DICOMweb.
pacs-send = Enviar…
pacs-go-back = Volver
pacs-confirm-send = Enviar
pacs-confirm = ¿Enviar { $studies ->
    [one] { $studies } estudio
    [many] { $studies } estudios
   *[other] { $studies } estudios
} ({ $files ->
    [one] { $files } archivo
    [many] { $files } archivos
   *[other] { $files } archivos
}) a { $server }?
pacs-busy = Todavía hay un envío en curso.
pacs-started = Enviando.
pacs-stopping = Deteniendo…
pacs-sending = Enviando { $done } de { $total } a { $server }…
pacs-sent-all = { $total ->
    [one] Se envió { $total } archivo a { $server }.
    [many] Se enviaron los { $total } archivos a { $server }.
   *[other] Se enviaron los { $total } archivos a { $server }.
}
pacs-sent-some = Se enviaron { $sent } de { $total } archivos a { $server }.
pacs-stopped = Detenido tras { $sent } de { $total ->
        [one] { $total } archivo
       *[other] { $total } archivos
    }.
pacs-stopped-because = Detenido tras { $sent } de { $total ->
        [one] { $total } archivo
       *[other] { $total } archivos
    }: { $reason }
pacs-warnings = { $count ->
    [one] { $count } archivo se almacenó con una advertencia.
    [many] { $count } archivos se almacenaron con una advertencia.
   *[other] { $count } archivos se almacenaron con una advertencia.
}
# A file that was not sent: its file name and why.
pacs-failed-line = { $name }: { $reason }
pacs-failed-more = Y { $count } más.

## Why one file was not sent.

pacs-reason-unreadable = No se pudo leer: { $reason }
pacs-reason-not-dicom = No es un archivo DICOM
pacs-reason-burned-in = Tiene datos del paciente incrustados en la imagen, por lo que no se envía anonimizado
pacs-reason-document = Un documento que no se puede anonimizar
pacs-reason-anonymise = No se pudo anonimizar: { $reason }
pacs-reason-not-accepted = El servidor no acepta { $class }
pacs-reason-convert = No se pudo convertir para el servidor: { $reason }
pacs-reason-resources = El servidor se ha quedado sin espacio o recursos
pacs-reason-mismatch = El servidor dice que los datos no coinciden con su tipo
pacs-reason-understand = El servidor no pudo leerlo
pacs-reason-class = El servidor no admite este tipo de objeto
pacs-reason-not-authorised = El servidor no lo permite
pacs-reason-status = El servidor lo rechazó (estado { $code })
pacs-reason-refused = El servidor lo rechazó
pacs-reason-too-large = Demasiado grande para el servidor
pacs-reason-http = El servidor lo rechazó (error { $status })

## Why sending stopped, or Verify failed.

pacs-fatal-network-off = La red está desactivada.
pacs-fatal-unknown-host = { $host } no es un nombre de host conocido.
pacs-fatal-connect = Nada responde en { $host }, puerto { $port }.
pacs-fatal-unreachable = No se puede acceder a { $host }.
pacs-fatal-timeout = { $host } no respondió a tiempo.
pacs-fatal-cancelled = Detenido.
pacs-fatal-called-ae = { $host } no responde al AE title { $ae }. Compruebe el AE title del servidor.
pacs-fatal-calling-ae = { $host } no conoce nuestro AE title { $ae }. Añádalo en el servidor.
pacs-fatal-busy = { $host } está ocupado. Inténtelo de nuevo más tarde.
pacs-fatal-rejected = { $host } rechazó la conexión.
pacs-fatal-aborted = { $host } cerró la conexión.
pacs-fatal-nothing-accepted = { $host } no acepta ninguno de estos tipos de objeto.
pacs-fatal-protocol = { $host } respondió de una forma que Specular no entiende.
pacs-fatal-auth = { $host } no aceptó el inicio de sesión.
pacs-fatal-http = { $host } respondió con el error { $status }.

## Teaching quiz.

palette-teaching-quiz = Cuestionario docente
palette-teaching-quiz-help = Recorrer las imágenes clave o un caso docente, con los hallazgos ocultos hasta que los muestre.
# "Case 2 of 5".
quiz-case = Caso { $n } de { $total }
quiz-hint-hidden = Espacio muestra los hallazgos · → caso siguiente · Esc termina
quiz-hint-revealed = Espacio los vuelve a ocultar · → caso siguiente · Esc termina
quiz-hidden = Hallazgos ocultos.
quiz-no-findings = No hay hallazgos registrados.
quiz-notes = Notas del caso: { $notes }
quiz-ended = Cuestionario terminado.
quiz-empty = No hay casos que mostrar. Marque imágenes clave con K o elija una carpeta de caso docente.
quiz-not-a-case = Esta carpeta no es un caso docente exportado por Specular.
quiz-pick-title = Elegir una carpeta de caso docente

## Teaching case folder (Export key images).

figure-keys-case = Carpeta de caso docente
figure-case-notes = Notas del caso
figure-case-findings = Hallazgos
figure-case-none = Ninguno registrado
figure-case-readme = Caso docente exportado por Specular. Los datos del paciente se han eliminado.
    images: las imágenes clave, con el texto incrustado tapado en negro.
    DICOM: copias anonimizadas y un manifiesto Key Object Selection «For Teaching».
    case.txt: las notas del caso y los hallazgos de cada imagen. case.json: lo mismo para scripts.
    Para Radiopaedia: suba la carpeta DICOM o las imágenes y copie las notas de case.txt.

## Search on a DIMSE server (C-FIND).
# machine-drafted, needs native review
pacs-find-not-supported = { $host } no responde a búsquedas (C-FIND).
pacs-find-status = { $host } rechazó la búsqueda (estado { $code }).

## Opening a series from a DIMSE server (C-GET, C-MOVE).
# machine-drafted, needs native review
pacs-retrieve-no-get = { $host } no envía series con C-GET.
    Elija C-MOVE en la configuración del servidor.
# machine-drafted, needs native review
pacs-retrieve-no-move = { $host } no acepta solicitudes C-MOVE.
# machine-drafted, needs native review
pacs-retrieve-unknown-destination = { $host } no conoce nuestro AE title { $ae }.
    Añádalo en el servidor con la dirección de este equipo y el puerto { $port }.
# machine-drafted, needs native review
pacs-retrieve-resources = { $host } se ha quedado sin recursos. Inténtelo de nuevo más tarde.
# machine-drafted, needs native review
pacs-retrieve-status = { $host } rechazó enviar la serie (estado { $code }).
# machine-drafted, needs native review
pacs-retrieve-no-matches = { $host } no tiene imágenes de esta serie.
# machine-drafted, needs native review
pacs-retrieve-roles = { $host } rechazó enviar imágenes por esta conexión.
    Elija C-MOVE en la configuración del servidor.
# machine-drafted, needs native review
pacs-retrieve-move-nothing = { $host } no pudo enviar las imágenes a este equipo.
    Compruebe la dirección, el puerto y el cortafuegos, y que el servidor conozca nuestro AE title { $ae }.
# machine-drafted, needs native review
pacs-retrieve-nothing = { $host } no pudo enviar { $count ->
    [one] la imagen
    [many] ninguna de las { $count } imágenes
   *[other] ninguna de las { $count } imágenes
} en un formato que Specular pueda leer.
# machine-drafted, needs native review
pacs-retrieve-listen = No se puede recibir en { $address }, puerto { $port }: { $reason }

## DICOM over TLS.
# machine-drafted, needs native review
pacs-fatal-tls-certificate = El certificado de { $host } no es de confianza.
    Su emisor debe estar entre los certificados de confianza del sistema.
pacs-fatal-tls = No hay conexión segura con { $host }.
    Compruebe que el puerto { $port } usa DICOM sobre TLS.
