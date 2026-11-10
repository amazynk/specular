# machine-drafted, needs native review
# Specular — Importação de câmeras, celulares e cartões de memória. Português (Brasil).
# O cartão só é lido; os arquivos só são apagados dele quando pedido, após uma cópia verificada.

palette-import = Importar fotos…
disabled-no-import-source = Nenhum cartão de memória, câmera ou celular conectado

## Configurações → Importação
settings-section-import = Importação
settings-import-on-card = Ao inserir um cartão
settings-import-on-card-help = O que acontece quando um cartão de memória, câmera ou celular com fotos é conectado.
settings-import-on-card-ask = Perguntar
settings-import-on-card-open = Abrir Importar
settings-import-on-card-nothing = Não fazer nada
settings-import-destination = Importar para
settings-import-destination-help = A pasta para onde vão as fotos e vídeos importados. Sem definição, vão para a sua pasta Imagens.
settings-import-destination-unset = Pasta Imagens
settings-import-folder-pattern = Pastas
settings-import-folder-pattern-help = As pastas criadas dentro dela: {"{"}year{"}"}, {"{"}month{"}"}, {"{"}day{"}"}, {"{"}camera{"}"} e {"{"}event{"}"}, separadas por /.
settings-import-rename-pattern = Renomear
settings-import-rename-pattern-help = Novos nomes de arquivo, como em Converter: {"{"}name{"}"}, {"{"}n:03{"}"}, {"{"}date:%Y%m%d{"}"}, {"{"}camera{"}"}, {"{"}event{"}"}. Vazio mantém os nomes da câmera.
settings-import-keep-pairs = Manter RAW e JPEG juntos
settings-import-keep-pairs-help = Um arquivo RAW e o JPEG tirado com ele vão para a mesma pasta com o mesmo nome, mesmo renomeados.
settings-import-low-res-videos = Importar cópias de vídeo em baixa resolução
settings-import-low-res-videos-help = Algumas câmeras gravam uma pequena cópia de prévia de cada vídeo (.LRV). Desativado, elas ficam no cartão.
settings-import-eject = Ejetar ao terminar
settings-import-eject-help = Ejeta o cartão quando tudo for copiado, para que possa ser removido.

## A oferta ao inserir um cartão
import-offer = Importar de { $name }?
import-offer-button = Importar

## A tela de importação
import-title = Importar
import-from = de { $name }
import-source-next = Próxima origem
import-close = Fechar
import-scanning = Procurando fotos novas… { $done } de { $total }
import-scanning-start = Procurando fotos novas…
import-none-new = Tudo em { $name } já foi importado antes.
import-empty = Não há fotos nem vídeos em { $name }.
import-no-source = Nenhum cartão de memória, câmera ou celular conectado.
import-summary = { $count ->
    [one] { $count } item novo
    [many] { $count } itens novos
   *[other] { $count } itens novos
} · { $size }
import-already = { $count } importados antes
import-unreadable = { $count ->
    [one] { $count } arquivo não pôde ser lido
    [many] { $count } arquivos não puderam ser lidos
   *[other] { $count } arquivos não puderam ser lidos
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
import-rename = Renomear
import-rename-hint = Vazio mantém os nomes da câmera
import-preview = Primeiro arquivo: { $path }
import-pattern-error = { $error }
import-keep-pairs = Manter RAW e JPEG juntos
import-eject = Ejetar ao terminar
import-delete = Apagar do cartão após importar
import-free = { $size } a copiar · { $free } de espaço livre
import-free-unknown = { $size } a copiar
import-no-space = Não há espaço livre suficiente em { $folder }: esta importação precisa de { $need } e só há { $free } livres. Libere espaço ou escolha outra pasta.
import-start = { $count ->
    [one] Importar { $count } item
    [many] Importar { $count } itens
   *[other] Importar { $count } itens
}
import-nothing-selected = Selecione as fotos a importar

import-confirm-delete = { $count ->
    [one] Depois de copiado e verificado, { $count } arquivo será apagado de { $name }. Não é possível desfazer.
    [many] Depois de copiados e verificados, { $count } arquivos serão apagados de { $name }. Não é possível desfazer.
   *[other] Depois de copiados e verificados, { $count } arquivos serão apagados de { $name }. Não é possível desfazer.
}
import-confirm-yes = Importar e apagar
import-confirm-no = Voltar

import-progress = Importando { $done } de { $total } · { $size }
import-progress-eta = Importando { $done } de { $total } · { $size } · { $time }
import-paused = Pausado · { $done } de { $total }
import-pause = Pausar
import-resume = Retomar
import-cancel = Cancelar
import-hide = Ocultar
import-stopping = Parando…
import-running = Já há uma importação em andamento.

import-done = { $count ->
    [one] { $count } arquivo importado
    [many] { $count } arquivos importados
   *[other] { $count } arquivos importados
} para { $folder }
import-done-existed = { $count ->
        [one] { $count } já estava lá
       *[other] { $count } já estavam lá
    }
import-done-deleted = { $count } apagados do cartão
import-done-failed = { $count ->
    [one] { $count } arquivo não pôde ser importado
    [many] { $count } arquivos não puderam ser importados
   *[other] { $count } arquivos não puderam ser importados
}
import-done-cancelled = Importação cancelada após { $count ->
        [one] { $count } arquivo
       *[other] { $count } arquivos
    }. Importar de novo continua de onde parou.
import-done-nothing = Nada novo foi importado.
import-ejected = { $name } pode ser removido.
import-eject-failed = Não foi possível ejetar { $name }: { $error }
import-failed-destination = A pasta { $folder } não pode ser usada: { $error }
import-ledger-failed = Não foi possível ler a lista de arquivos importados ({ $error }); arquivos já importados podem ser oferecidos de novo.

## Leitor de tela
import-a11y-group = Importar
import-a11y-tile = { $name }, { $date }
import-a11y-tile-selected = { $name }, { $date }, selecionado
