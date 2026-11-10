# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Limite das cópias locais
settings-library-local-copies-limit-help = O espaço máximo que as cópias locais podem ocupar na pasta de dados do Specular.

local-keep-menu = Manter uma cópia local…
local-remove-menu = Remover cópia local…
local-keep-location-menu = Manter uma cópia local deste local…
local-remove-location-menu = Remover cópia local deste local…
local-keep-album-menu = Manter uma cópia local deste álbum…
local-remove-album-menu = Remover cópia local deste álbum…
local-remove-held-elsewhere = Estas cópias pertencem a um local ou álbum mantido. Remova-as lá.

local-keep-ask = Manter uma cópia local de { $item }?
local-keep-detail = { $count ->
    [one] 1 foto.
    [many] { $count } fotos.
   *[other] { $count } fotos.
}
    Originais: os arquivos como estão.
    Proxies: raws com 2.560 pixels, o bastante para revelar. Os outros arquivos são copiados inteiros.
    As cópias vão para a pasta de dados do Specular. Os originais não mudam.
local-keep-originals = Manter originais
local-keep-proxies = Manter proxies
local-keeping = Copiando { $item }…
local-kept = { $item }: { $count ->
    [one] 1 cópia local
    [many] { $count } cópias locais
   *[other] { $count } cópias locais
}, { $size }.
local-kept-full = { $count ->
    [one] 1 foto não coube no limite de tamanho.
    [many] { $count } fotos não couberam no limite de tamanho.
   *[other] { $count } fotos não couberam no limite de tamanho.
}
local-kept-unreachable = { $count ->
    [one] Não foi possível ler 1 foto.
    [many] Não foi possível ler { $count } fotos.
   *[other] Não foi possível ler { $count } fotos.
}

local-remove-ask = Remover a cópia local de { $item }?
local-remove-detail = As cópias na pasta de dados do Specular são apagadas. Os originais não são tocados. As alterações pendentes são mantidas.
local-remove-ok = Remover cópia local
local-removed = Cópia local de { $item } removida, { $size } liberados.

local-banner-offline = { $location } está off-line. Você está trabalhando em cópias locais.
local-banner-offline-many = { $count } locais estão off-line. Você está trabalhando em cópias locais.
local-banner-waiting = { $count ->
    [one] 1 alteração aguarda o original.
    [many] { $count } alterações aguardam os originais.
   *[other] { $count } alterações aguardam os originais.
}
local-banner-conflicts = { $count ->
    [one] 1 arquivo mudou enquanto você estava off-line.
    [many] { $count } arquivos mudaram enquanto você estava off-line.
   *[other] { $count } arquivos mudaram enquanto você estava off-line.
}
local-review = Revisar…
local-sidebar-originals = cópia local
local-sidebar-proxies = proxies locais

local-synced = { $count ->
    [one] 1 alteração gravada no original.
    [many] { $count } alterações gravadas nos originais.
   *[other] { $count } alterações gravadas nos originais.
}
local-sync-failed = { $count ->
    [one] Não foi possível gravar 1 alteração. Ela continua aguardando.
    [many] Não foi possível gravar { $count } alterações. Elas continuam aguardando.
   *[other] Não foi possível gravar { $count } alterações. Elas continuam aguardando.
}

local-conflict-title = { $file } mudou enquanto você estava off-line. Qual versão manter: { $what }?
local-conflict-yours = A sua: { $text }
local-conflict-theirs = No disco: { $text }
local-conflict-later = Cancelar decide depois. Nada é sobrescrito.
local-keep-mine = Manter a sua
local-keep-theirs = Manter a do arquivo
local-what-marks = classificação
local-what-keywords = palavras-chave
local-what-fields = campos IPTC
local-what-edit = edição
local-stars = { $count ->
    [0] Sem estrelas
    [one] 1 estrela
    [many] { $count } estrelas
   *[other] { $count } estrelas
}
local-label = { $label ->
    [1] Vermelho
    [2] Amarelo
    [3] Verde
   *[4] Azul
}
local-pick = Escolhida
local-reject = Rejeitada
local-no-keywords = sem palavras-chave
local-changed-elsewhere = alterado em outro lugar
local-photo-changed = a própria foto mudou
local-edit-here = editada aqui, { $when }
local-edit-elsewhere = editada em outro lugar, { $when }
local-edit-none = sem edição

local-export-ask = { $count ->
    [one] 1 foto está off-line. Exportá-la do proxy?
    [many] { $count } fotos estão off-line. Exportá-las dos proxies?
   *[other] { $count } fotos estão off-line. Exportá-las dos proxies?
}
local-export-detail = Os proxies têm 2.560 pixels no lado maior. Os originais dão o tamanho completo.
local-export-proxy = Exportar dos proxies
local-export-wait = Aguardar os originais
local-export-waiting = A exportação começa quando os originais voltarem.

local-panel-title = Cópias locais
local-panel-used = Usando { $used } de { $limit }.
local-panel-originals = { $count ->
    [one] 1 original
    [many] { $count } originais
   *[other] { $count } originais
}
local-panel-proxies = { $count ->
    [one] 1 foto, raws como proxies
    [many] { $count } fotos, raws como proxies
   *[other] { $count } fotos, raws como proxies
}
local-panel-remove = Remover…
local-panel-footer = Guardadas em { $folder }. Os originais nunca são alterados.
local-panel-keywords = off-line offline cópias locais proxies espaço
