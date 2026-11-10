# machine-drafted, needs native review
# Specular — Importação de câmaras, telemóveis e cartões de memória. Português (Portugal).
# O cartão só é lido; os ficheiros só são eliminados dele quando pedido, após uma cópia verificada.

palette-import = Importar fotografias…
disabled-no-import-source = Nenhum cartão de memória, câmara ou telemóvel ligado

## Definições → Importação
settings-section-import = Importação
settings-import-on-card = Ao inserir um cartão
settings-import-on-card-help = O que acontece quando é ligado um cartão de memória, câmara ou telemóvel com fotografias.
settings-import-on-card-ask = Perguntar
settings-import-on-card-open = Abrir Importar
settings-import-on-card-nothing = Não fazer nada
settings-import-destination = Importar para
settings-import-destination-help = A pasta para onde vão as fotografias e vídeos importados. Sem definição, vão para a sua pasta Imagens.
settings-import-destination-unset = Pasta Imagens
settings-import-folder-pattern = Pastas
settings-import-folder-pattern-help = As pastas criadas dentro dela: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} e {"{"}event{"}"}, separadas por /.
settings-import-rename-pattern = Mudar o nome
settings-import-rename-pattern-help = Novos nomes de ficheiro, como em Converter: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Vazio mantém os nomes da câmara.
settings-import-keep-pairs = Manter RAW e JPEG juntos
settings-import-keep-pairs-help = Um ficheiro RAW e o JPEG tirado com ele vão para a mesma pasta com o mesmo nome, mesmo com o nome mudado.
settings-import-low-res-videos = Importar cópias de vídeo de baixa resolução
settings-import-low-res-videos-help = Algumas câmaras gravam uma pequena cópia de pré-visualização de cada vídeo (.LRV). Desativado, ficam no cartão.
settings-import-eject = Ejetar no fim
settings-import-eject-help = Ejeta o cartão quando tudo estiver copiado, para que possa ser retirado.

## A proposta ao inserir um cartão
import-offer = Importar de { $name }?
import-offer-button = Importar

## A vista de importação
import-title = Importar
import-from = de { $name }
import-source-next = Origem seguinte
import-close = Fechar
import-scanning = A procurar fotografias novas… { $done } de { $total }
import-scanning-start = A procurar fotografias novas…
import-none-new = Tudo em { $name } já foi importado antes.
import-empty = Não há fotografias nem vídeos em { $name }.
import-no-source = Nenhum cartão de memória, câmara ou telemóvel ligado.
import-summary = { $count ->
    [one] { $count } item novo
    [many] { $count } itens novos
   *[other] { $count } itens novos
} · { $size }
import-already = { $count } importados antes
import-unreadable = { $count ->
    [one] Não foi possível ler { $count } ficheiro
    [many] Não foi possível ler { $count } ficheiros
   *[other] Não foi possível ler { $count } ficheiros
}
import-selected = { $count } de { $total } selecionados
import-select-all = Selecionar tudo
import-select-none = Não selecionar nada
import-badge-pair = RAW+JPEG
import-badge-raw = RAW
import-badge-video = Vídeo

import-destination = Importar para
import-choose = Escolher…
import-folder-pattern = Pastas
import-event = Evento
import-event-hint = Opcional, para {"{"}event{"}"}
import-rename = Mudar o nome
import-rename-hint = Vazio mantém os nomes da câmara
import-preview = Primeiro ficheiro: { $path }
import-pattern-error = { $error }
import-keep-pairs = Manter RAW e JPEG juntos
import-eject = Ejetar no fim
import-delete = Eliminar do cartão após importar
import-free = { $size } a copiar · { $free } de espaço livre
import-free-unknown = { $size } a copiar
import-no-space = Não há espaço livre suficiente em { $folder }: esta importação precisa de { $need } e só há { $free } livres. Liberte espaço ou escolha outra pasta.
import-start = { $count ->
    [one] Importar { $count } item
    [many] Importar { $count } itens
   *[other] Importar { $count } itens
}
import-nothing-selected = Selecione as fotografias a importar

import-confirm-delete = { $count ->
    [one] Depois de copiado e verificado, { $count } ficheiro será eliminado de { $name }. Não é possível anular.
    [many] Depois de copiados e verificados, { $count } ficheiros serão eliminados de { $name }. Não é possível anular.
   *[other] Depois de copiados e verificados, { $count } ficheiros serão eliminados de { $name }. Não é possível anular.
}
import-confirm-yes = Importar e eliminar
import-confirm-no = Voltar

import-progress = A importar { $done } de { $total } · { $size }
import-progress-eta = A importar { $done } de { $total } · { $size } · { $time }
import-paused = Em pausa · { $done } de { $total }
import-pause = Pausa
import-resume = Retomar
import-cancel = Cancelar
import-hide = Ocultar
import-stopping = A parar…
import-running = Já há uma importação em curso.

import-done = { $count ->
    [one] { $count } ficheiro importado
    [many] { $count } ficheiros importados
   *[other] { $count } ficheiros importados
} para { $folder }
import-done-existed = { $count ->
        [one] { $count } já lá estava
       *[other] { $count } já lá estavam
    }
import-done-deleted = { $count } eliminados do cartão
import-done-failed = { $count ->
    [one] Não foi possível importar { $count } ficheiro
    [many] Não foi possível importar { $count } ficheiros
   *[other] Não foi possível importar { $count } ficheiros
}
import-done-cancelled = Importação cancelada após { $count ->
        [one] { $count } ficheiro
       *[other] { $count } ficheiros
    }. Importar de novo continua onde parou.
import-done-nothing = Nada de novo foi importado.
import-ejected = { $name } pode ser retirado.
import-eject-failed = Não foi possível ejetar { $name }: { $error }
import-failed-destination = A pasta { $folder } não pode ser utilizada: { $error }
import-ledger-failed = Não foi possível ler a lista de ficheiros importados ({ $error }); ficheiros já importados podem ser propostos de novo.

## Leitor de ecrã
import-a11y-group = Importar
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, selecionado
