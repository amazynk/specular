# machine-drafted, needs native review
# Local copies for offline editing (the Library).

settings-library-local-copies-limit = Limite das cópias locais
settings-library-local-copies-limit-help = O espaço máximo que as cópias locais podem ocupar na pasta de dados do Specular.

local-keep-menu = Manter uma cópia local…
local-remove-menu = Remover cópia local…
local-keep-location-menu = Manter uma cópia local desta localização…
local-remove-location-menu = Remover cópia local desta localização…
local-keep-album-menu = Manter uma cópia local deste álbum…
local-remove-album-menu = Remover cópia local deste álbum…
local-remove-held-elsewhere = Estas cópias pertencem a uma localização ou álbum mantido. Remova-as aí.

local-keep-ask = Manter uma cópia local de { $item }?
local-keep-detail = { $count ->
    [one] 1 fotografia.
    [many] { $count } fotografias.
   *[other] { $count } fotografias.
}
    Originais: os ficheiros tal como estão.
    Proxies: raws com 2560 píxeis, o suficiente para revelar. Os outros ficheiros são copiados inteiros.
    As cópias vão para a pasta de dados do Specular. Os originais não são alterados.
local-keep-originals = Manter originais
local-keep-proxies = Manter proxies
local-keeping = A copiar { $item }…
local-kept = { $item }: { $count ->
    [one] 1 cópia local
    [many] { $count } cópias locais
   *[other] { $count } cópias locais
}, { $size }.
local-kept-full = { $count ->
    [one] 1 fotografia não coube no limite de tamanho.
    [many] { $count } fotografias não couberam no limite de tamanho.
   *[other] { $count } fotografias não couberam no limite de tamanho.
}
local-kept-unreachable = { $count ->
    [one] Não foi possível ler 1 fotografia.
    [many] Não foi possível ler { $count } fotografias.
   *[other] Não foi possível ler { $count } fotografias.
}

local-remove-ask = Remover a cópia local de { $item }?
local-remove-detail = As cópias na pasta de dados do Specular são apagadas. Os originais não são tocados. As alterações pendentes são mantidas.
local-remove-ok = Remover cópia local
local-removed = Cópia local de { $item } removida, { $size } libertados.

local-banner-offline = { $location } está offline. Está a trabalhar em cópias locais.
local-banner-offline-many = { $count } localizações estão offline. Está a trabalhar em cópias locais.
local-banner-waiting = { $count ->
    [one] 1 alteração aguarda o original.
    [many] { $count } alterações aguardam os originais.
   *[other] { $count } alterações aguardam os originais.
}
local-banner-conflicts = { $count ->
    [one] 1 ficheiro mudou enquanto estava offline.
    [many] { $count } ficheiros mudaram enquanto estava offline.
   *[other] { $count } ficheiros mudaram enquanto estava offline.
}
local-review = Rever…
local-sidebar-originals = cópia local
local-sidebar-proxies = proxies locais

local-synced = { $count ->
    [one] 1 alteração escrita no original.
    [many] { $count } alterações escritas nos originais.
   *[other] { $count } alterações escritas nos originais.
}
local-sync-failed = { $count ->
    [one] Não foi possível escrever 1 alteração. Continua a aguardar.
    [many] Não foi possível escrever { $count } alterações. Continuam a aguardar.
   *[other] Não foi possível escrever { $count } alterações. Continuam a aguardar.
}

local-conflict-title = { $file } mudou enquanto estava offline. Que versão manter: { $what }?
local-conflict-yours = A sua: { $text }
local-conflict-theirs = No disco: { $text }
local-conflict-later = Cancelar decide mais tarde. Nada é substituído.
local-keep-mine = Manter a sua
local-keep-theirs = Manter a do ficheiro
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
local-changed-elsewhere = alterado noutro lado
local-photo-changed = a própria fotografia mudou
local-edit-here = editada aqui, { $when }
local-edit-elsewhere = editada noutro lado, { $when }
local-edit-none = sem edição

local-export-ask = { $count ->
    [one] 1 fotografia está offline. Exportá-la do proxy?
    [many] { $count } fotografias estão offline. Exportá-las dos proxies?
   *[other] { $count } fotografias estão offline. Exportá-las dos proxies?
}
local-export-detail = Os proxies têm 2560 píxeis no lado maior. Os originais dão o tamanho completo.
local-export-proxy = Exportar dos proxies
local-export-wait = Aguardar os originais
local-export-waiting = A exportação começa quando os originais voltarem.

local-panel-title = Cópias locais
local-panel-used = A usar { $used } de { $limit }.
local-panel-originals = { $count ->
    [one] 1 original
    [many] { $count } originais
   *[other] { $count } originais
}
local-panel-proxies = { $count ->
    [one] 1 fotografia, raws como proxies
    [many] { $count } fotografias, raws como proxies
   *[other] { $count } fotografias, raws como proxies
}
local-panel-remove = Remover…
local-panel-footer = Guardadas em { $folder }. Os originais nunca são alterados.
local-panel-keywords = offline cópias locais proxies espaço
