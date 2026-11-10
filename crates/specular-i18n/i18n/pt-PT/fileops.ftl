# machine-drafted, needs native review
# Operações de ficheiros: mudar o nome, mudar o nome em lote, copiar e mover para uma pasta,
# rotação guardada no ficheiro, anular e refazer, imagem de fundo, partilhar, enviar por
# e-mail, exportar e importar definições.

## Paleta, folha de atalhos e menus

palette-undo = Anular
palette-undo-help = Anular a última alteração a ficheiros: mover para o Lixo, mudar o nome, copiar ou mover, rodar.
palette-redo = Refazer
palette-redo-help = Refazer o que a última anulação reverteu.
palette-rename = Mudar o nome…
palette-rename-help = Dar um novo nome ao ficheiro; os ficheiros sidecar e o par RAW+JPEG acompanham-no.
palette-batch-rename = Mudar o nome de ficheiros…
palette-batch-rename-help = Mudar o nome dos ficheiros selecionados, ou da pasta inteira, a partir de um padrão, com pré-visualização.
palette-copy-to-folder = Copiar para pasta…
palette-copy-to-folder-help = Copiar o ficheiro ou a seleção para outra pasta.
palette-move-to-folder = Mover para pasta…
palette-move-to-folder-help = Mover o ficheiro ou a seleção para outra pasta.
palette-rotate-file-left = Rodar ficheiro para a esquerda
palette-rotate-file-left-help = Rodar o próprio ficheiro um quarto de volta para a esquerda, sem perda quando o formato o permite.
palette-rotate-file-right = Rodar ficheiro para a direita
palette-rotate-file-right-help = Rodar o próprio ficheiro um quarto de volta para a direita, sem perda quando o formato o permite.
palette-set-wallpaper-help = Usar a imagem como fundo do ambiente de trabalho.
palette-share = Partilhar…
palette-share-help = Enviar o ficheiro para outra app através do menu de partilha do sistema.
palette-send-by-email = Enviar por e-mail…
palette-send-by-email-help = Criar um novo e-mail na sua app de correio com o ficheiro anexado.
# machine-drafted, needs native review
palette-share-with-zynk = Partilhar com o Zynk…
palette-share-with-zynk-help = Criar uma ligação web para o ficheiro na app Zynk.
palette-send-with-zynk = Enviar com o Zynk…
palette-send-with-zynk-help = Enviar o ficheiro a um contacto ou a um dos seus dispositivos com o Zynk.
palette-export-settings = Exportar definições…
palette-export-settings-help = Guardar todas as definições e os seus atalhos de teclado num ficheiro.
palette-import-settings = Importar definições…
palette-import-settings-help = Ler definições de um ficheiro, depois de mostrar o que mudaria.

disabled-in-archive-file = Esta página está dentro de um arquivo e não tem ficheiro próprio
disabled-not-rotatable = Este tipo de ficheiro não pode registar uma rotação
disabled-not-on-this-desktop = O seu ambiente de trabalho não oferece isto

## Definições

settings-browsing-rotate-trim-edges = Aparar margens de JPEG para rodar sem perda
settings-browsing-rotate-trim-edges-help = Um JPEG cujo tamanho não é múltiplo de 8 ou 16 píxeis só pode ser rodado sem perda cortando os últimos píxeis. Sem esta opção, é alterada a etiqueta de orientação, e o corte é proposto de cada vez.
settings-browsing-rotate-keep-modified = Manter a data de modificação ao rodar
settings-browsing-rotate-keep-modified-help = Um ficheiro rodado mantém a data da última alteração e fica no mesmo lugar quando ordenado por data.
settings-browsing-recent-folders = Pastas recentes
settings-browsing-recent-folders-help = Pastas usadas por último para copiar e mover.

## Janelas

fileop-cancel = Cancelar
rename-title = Mudar o nome
rename-field-label = Nome
rename-button = Mudar o nome
rename-sidecars-follow = Também com nome alterado: { $files }
rename-extension-changed = Mudar a extensão não converte o ficheiro; outras apps podem não o abrir.
rename-problem-empty = Escreva um nome.
rename-problem-invalid-char = Os nomes não podem conter “{ $char }”.
rename-problem-reserved = “{ $name }” é um nome que o Windows reserva para dispositivos.
rename-problem-too-long = Esse nome é demasiado longo; os nomes podem ter até 255 bytes.
rename-problem-dot = “.” e “..” não podem ser usados como nomes.
rename-problem-trailing = Os nomes não podem terminar com um espaço ou um ponto.
rename-problem-exists = Outro ficheiro já tem este nome.
rename-problem-read-only = Esta pasta não pode ser alterada.
rename-problem-duplicate = Dois ficheiros ficariam com este nome.

batch-rename-title =
    { $count ->
        [one] Mudar o nome de { $count } ficheiro
       *[other] Mudar o nome de { $count } ficheiros
    }
batch-rename-field-label = Padrão
batch-rename-tokens = Use { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } e { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } ficheiro vai mudar de nome
       *[other] { $count } ficheiros vão mudar de nome
    }{ $problems ->
        [0] .
        [one] ; { $problems } não pode e fica como está.
       *[other] ; { $problems } não podem e ficam como estão.
    }
batch-rename-reading = A ler datas e câmaras…
batch-rename-preview = Pré-visualização
batch-rename-unchanged = mantém o nome
batch-rename-more =
    { $count ->
        [one] Mais { $count } abaixo
       *[other] Mais { $count } abaixo
    }

transfer-copy-title-one = Copiar “{ $file }” para uma pasta
transfer-copy-title =
    { $count ->
        [one] Copiar { $count } ficheiro para uma pasta
       *[other] Copiar { $count } ficheiros para uma pasta
    }
transfer-move-title-one = Mover “{ $file }” para uma pasta
transfer-move-title =
    { $count ->
        [one] Mover { $count } ficheiro para uma pasta
       *[other] Mover { $count } ficheiros para uma pasta
    }
transfer-recent = Pastas recentes
transfer-folder = Pasta
transfer-choose-folder = Escolher uma pasta…
transfer-copy-dialog = Copiar para pasta
transfer-move-dialog = Mover para pasta
transfer-sidecars =
    { $count ->
        [one] { $count } ficheiro sidecar vai junto.
       *[other] { $count } ficheiros sidecar vão junto.
    }
transfer-taken =
    { $count ->
        [one] { $count } nome já está a ser usado em { $folder }.
       *[other] { $count } nomes já estão a ser usados em { $folder }.
    }
transfer-when-taken = Quando um nome já está a ser usado
transfer-skip = Ignorar
transfer-keep-both = Manter ambos
transfer-replace-option = Substituir
transfer-copy = Copiar
transfer-move = Mover
transfer-replace-title =
    { $count ->
        [one] Substituir { $count } ficheiro em { $folder }?
       *[other] Substituir { $count } ficheiros em { $folder }?
    }
transfer-replace-note = Os ficheiros que lá estão vão para o Lixo. Anular repõe-nos.
transfer-replace = Substituir
transfer-back = Voltar
transfer-stop = Parar

settings-file-import-title = Importar definições
settings-file-import-dialog = Importar definições
settings-file-export-dialog = Exportar definições
settings-file-import-changes = Alterações
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = sem tecla
settings-file-import-unknown = Não é uma definição desta versão; ignorada.
settings-file-import-keymap = Atalhos de teclado
settings-file-import-nothing = Nada mudaria: estas definições já estão em vigor.
settings-file-import-summary =
    { $count ->
        [one] { $count } alteração. A importação pode ser anulada em Definições.
       *[other] { $count } alterações. A importação pode ser anulada em Definições.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } entrada parece uma palavra-passe ou chave e nunca é importada.
       *[other] { $count } entradas parecem palavras-passe ou chaves e nunca são importadas.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } comando do utilizador não é importado: os comandos iniciam programas. Se confiar nele, copie-o para o ficheiro de definições.
       *[other] { $count } comandos do utilizador não são importados: os comandos iniciam programas. Se confiar neles, copie-os para o ficheiro de definições.
    }
settings-file-import-button = Importar

## Resultados

fileop-renamed = Nome alterado para { $file }
fileop-renamed-count =
    { $count ->
        [one] Nome de { $count } ficheiro alterado
       *[other] Nome de { $count } ficheiros alterado
    }
fileop-copied =
    { $count ->
        [one] { $count } ficheiro copiado para { $folder }
       *[other] { $count } ficheiros copiados para { $folder }
    }
fileop-moved =
    { $count ->
        [one] { $count } ficheiro movido para { $folder }
       *[other] { $count } ficheiros movidos para { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } ignorado: o nome já está a ser usado lá.
       *[other] { $count } ignorados: os nomes já estão a ser usados lá.
    }
fileop-stopped = Parado; o que foi feito mantém-se.
fileop-failed-file =
    { $count ->
        [one] Não foi possível copiar ou mover { $file }: { $reason }
       *[other] Não foi possível copiar ou mover { $count } ficheiros; o primeiro, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] A copiar { $count } ficheiro… { PERCENT($percent) }
       *[other] A copiar { $count } ficheiros… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] A mover { $count } ficheiro… { PERCENT($percent) }
       *[other] A mover { $count } ficheiros… { PERCENT($percent) }
    }
fileop-done-status = Concluído
fileop-busy = A última alteração de ficheiros ainda está em curso; tente novamente dentro de momentos.
fileop-nothing-to-redo = Nada para refazer
fileop-undone = Anulado: { $what }
fileop-redone = Refeito: { $what }
fileop-kind-rename = mudança de nome
fileop-kind-batch-rename = mudança de nome de vários ficheiros
fileop-kind-move = mover para pasta
fileop-kind-copy = copiar para pasta
fileop-kind-rotate = rotação
rename-failed = Não foi possível mudar o nome: { $reason }

undo-problem-changed = Não é possível anular: { $file } foi alterado entretanto, e anular perderia essa alteração.
undo-problem-missing = Não é possível anular: { $file } já não existe.
undo-problem-taken = Não é possível anular: outro ficheiro chama-se agora { $file }.
undo-problem-io = Não é possível anular a alteração a { $file }: { $reason }

rotate-done-lossless = { $file } rodado sem qualquer perda.
rotate-done-lossless-trimmed = { $file } rodado sem qualquer perda; a margem foi aparada para { $width } × { $height }.
rotate-done-tag = { $file } rodado alterando a etiqueta de orientação.
rotate-done-sidecar = Rotação de { $file } guardada em { $sidecar }; o ficheiro RAW em si nunca é alterado.
rotate-tag-unreadable = Não foi possível ler os dados da imagem o suficiente para a rodar.
rotate-tag-multi-picture = Contém mais de uma imagem, e rodar a primeira estragaria as outras.
rotate-tag-trailer = Há dados depois da imagem que a rotação deslocaria.
rotate-tag-not-aligned = Rodar os píxeis sem perda apará-la-ia para { $width } × { $height }.
rotate-trim-offer = Aparar e rodar
rotate-problem-unsupported = { $file } é um tipo de ficheiro que não pode registar uma rotação.
rotate-problem-too-large = { $file } é demasiado grande para rodar.
rotate-problem-damaged = { $file } está danificado onde a orientação é guardada: { $reason }
rotate-problem-io = Não foi possível rodar { $file }: { $reason }

share-failed = Não foi possível partilhar: { $reason }
email-failed = Não foi possível abrir a sua app de correio: { $reason }
# machine-drafted, needs native review
zynk-opening = A abrir o Zynk…
zynk-failed = Não foi possível contactar o Zynk: { $reason }
zynk-get-title-send = “Enviar com o Zynk” precisa da app Zynk
zynk-get-title-share = “Partilhar com o Zynk” precisa da app Zynk
zynk-get-about = O Zynk é uma app gratuita para Mac, Windows, Linux, iPhone e Android. Envia fotografias, vídeos e pastas inteiras diretamente para os seus outros dispositivos e para outras pessoas, de qualquer tamanho. As transferências são encriptadas ponto a ponto e continuam onde pararam se a ligação cair. Para um destinatário sem o Zynk, cria uma ligação web a partir da qual pode transferir os ficheiros no navegador.
zynk-get-macos = Obtenha o Zynk na Mac App Store. Depois de instalado, o { -app-name } entrega-lhe os seus ficheiros diretamente.
zynk-get-windows = Obtenha o Zynk na Microsoft Store. Depois de instalado, o { -app-name } entrega-lhe os seus ficheiros diretamente.
zynk-get-linux = O Zynk para Linux é uma AppImage, sem nada para instalar: transfira-a, coloque-a em ~/Applications e torne-a executável (nas Propriedades ou com chmod +x). O { -app-name } encontra-a aí, e em qualquer outro sítio enquanto o Zynk estiver aberto.
zynk-get = Obter o Zynk
zynk-send-old-windows = “Enviar com o Zynk” requer o Windows 10 versão 2004 ou posterior. “Partilhar com o Zynk” funciona neste Windows.
settings-file-export-done = Definições guardadas em { $file }
settings-file-export-failed = Não foi possível guardar as definições: { $reason }
settings-file-import-unreadable = Não foi possível ler { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [one] { $count } alteração importada
       *[other] { $count } alterações importadas
    }
