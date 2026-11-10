# machine-drafted, needs native review
# Specular — Downloads: what is being fetched from a server (shown beside the toolbar, in
# the gallery's bottom bar, or as a small indicator) and the panel listing it. Português (Brasil).

## The panel

# The panel's title.
net-title = Downloads
# The palette entry that opens the panel, and the indicator's tooltip.
net-show = Mostrar downloads
# The panel when nothing is downloading any more.
net-empty = Nada está sendo baixado.
# Cancels one download.
net-stop = Parar

## What is downloading (row titles). $host is a server's name ("tiles.example.org").

net-what-data = Dados de { $host }
# Tiles of a remote image (a cloud GeoTIFF, an OMERO image).
net-what-image-tiles = Blocos de imagem de { $host }
net-what-point-cloud = Nuvem de pontos de { $host }
net-what-map-tiles = Blocos de mapa de { $host }
# A map service's capabilities, features or map images.
net-what-map-service = Serviço de mapas em { $host }
net-what-dicom = Dados DICOM de { $host }
# $series is the series' description ("CT Abdomen 5 mm").
net-what-dicom-series = Série DICOM { $series } de { $host }
# Studies being sent to a PACS; { $host } is its host and port.
# machine-drafted, needs native review
net-what-dicom-send = Envio de estudos para { $host }
net-what-omero = Dados OMERO de { $host }
net-what-zarr-chunks = Blocos Zarr de { $host }
net-what-zarr-metadata = Metadados Zarr de { $host }

## Remote Zarr stores ($address: the store's address, without any query or password).

zarr-open-failed = Não é possível abrir { $address }: { $reason }
# A Local AI model; $name is its name.
net-what-model = Modelo { $name }
net-what-update = Procura de atualizações

## Figures

# How much of how much: $done and $total are formatted sizes ("45 MB", "120 MB").
net-bytes-of = { $done } de { $total }
# A speed: $size is a formatted size ("2.1 MB").
net-speed = { $size }/s
# Many requests: $done and $total are formatted numbers; $count is $total, plain, for the
# plural.
net-requests = { $done } de { $total } { $count ->
    [one] solicitação
    [many] solicitações
   *[other] solicitações
}
# A DICOM series retrieved image by image: $done and $total are formatted numbers;
# $count is $total, plain, for the plural.
# machine-drafted, needs native review
net-images = { $done } de { $total } { $count ->
    [one] imagem
    [many] imagens
   *[other] imagens
}
# Images the server could not send ($n formatted, $count plain).
# machine-drafted, needs native review
net-images-failed = { $n } com falha
# Images sent with a warning.
# machine-drafted, needs native review
net-images-warning = { $n } { $count ->
    [one] aviso
    [many] avisos
   *[other] avisos
}
# Two parts of a line joined ("45 MB of 120 MB · 2.1 MB/s").
net-join = { $first } · { $second }

## Status

# Running, the speed not measured yet.
net-starting = Iniciando…
# Running, no time left known (many requests, or an unknown size).
net-running = Baixando
net-done = Concluído
# Done, but $count of its requests failed.
net-done-failed = Concluído, { $count } com falha
# Cancelled with Stop.
net-stopped = Parado
# $reason is one of the reasons below.
net-failed = Falhou: { $reason }
net-reason-offline = sem conexão
net-reason-timeout = o servidor não respondeu a tempo
# $code is the HTTP status number.
net-reason-status = o servidor respondeu { $code }
net-reason-auth = login recusado
net-reason-other = o download foi interrompido

## The indicator

# More downloads than shown in place.
net-more = mais { $count }
# The indicator's name for screen readers: $status is the newest download and its figures.
net-chip-name = Downloads: { $status }

## Announcements (screen readers)

# $what is a row title ("Model clip-vit-b32").
net-started = Baixando { $what }
net-finished = Baixado: { $what }
net-failed-announce = Falha no download: { $what }
net-stopped-announce = Download parado: { $what }

## Sem rede (Configurações › Privacidade)

# machine-drafted, needs native review
# Shown where a network feature would be (update check, model downloads, servers, maps).
network-off = A rede está desligada em Configurações › Privacidade.
# The panel under the switch while the network is off.
privacy-off-title = A rede está desligada.
privacy-off-what = Verificação de atualizações, downloads de modelos, servidores DICOM, OMERO e Zarr, imagens na nuvem e mapas estão desligados.
# What holds the network off when it is not the switch. $file is the policy file's path.
privacy-held-policy = Definido pelo seu administrador em { $file }. Este botão não consegue religar a rede.
# $what is --no-network or SPECULAR_NO_NETWORK.
privacy-held-run = Definido para esta execução por { $what }. Este botão não consegue religar a rede.

## Open address… (File)

# machine-drafted, needs native review
address-title = Abrir endereço
# machine-drafted, needs native review
address-intro = Um repositório Zarr, um GeoTIFF na nuvem (.tif), uma nuvem de pontos COPC (.copc.laz), um item ou catálogo STAC (.json) ou um servidor DICOMweb.
address-label = Endereço (http, https ou s3)
address-recent = Recentes
address-open = Abrir
address-network-off = Não é possível abrir endereços: a rede está desligada em Configurações › Privacidade.
address-problem-empty = Digite ou cole um endereço.
address-problem-scheme = O endereço deve começar com http://, https:// ou s3://.
address-problem-host = O endereço não tem nome de servidor ou tem espaços.
# machine-drafted, needs native review
address-problem-kind = O { -app-name } não abre este endereço. Ele abre um repositório .zarr, um .tif, um .copc.laz, um .json STAC ou um servidor DICOMweb.
