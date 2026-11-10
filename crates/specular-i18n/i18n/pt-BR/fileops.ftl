# machine-drafted, needs native review
# Operações de arquivo: renomear, renomear em lote, copiar e mover para uma pasta, rotação
# salva no arquivo, desfazer e refazer, papel de parede, compartilhar, enviar por e-mail,
# exportar e importar configurações.

## Paleta, folha de atalhos e menus

palette-undo = Desfazer
palette-undo-help = Desfazer a última alteração em arquivos: mover para a Lixeira, renomear, copiar ou mover, girar.
palette-redo = Refazer
palette-redo-help = Refazer o que o último desfazer reverteu.
palette-rename = Renomear…
palette-rename-help = Dar um novo nome ao arquivo; arquivos sidecar e o par RAW+JPEG acompanham.
palette-batch-rename = Renomear arquivos…
palette-batch-rename-help = Renomear os arquivos selecionados, ou a pasta inteira, a partir de um padrão, com prévia.
palette-copy-to-folder = Copiar para pasta…
palette-copy-to-folder-help = Copiar o arquivo ou a seleção para outra pasta.
palette-move-to-folder = Mover para pasta…
palette-move-to-folder-help = Mover o arquivo ou a seleção para outra pasta.
palette-rotate-file-left = Girar arquivo para a esquerda
palette-rotate-file-left-help = Girar o próprio arquivo um quarto de volta para a esquerda, sem perda quando o formato permite.
palette-rotate-file-right = Girar arquivo para a direita
palette-rotate-file-right-help = Girar o próprio arquivo um quarto de volta para a direita, sem perda quando o formato permite.
palette-set-wallpaper-help = Usar a imagem como plano de fundo da área de trabalho.
palette-share = Compartilhar…
palette-share-help = Enviar o arquivo para outro app pelo menu de compartilhamento do sistema.
palette-send-by-email = Enviar por e-mail…
palette-send-by-email-help = Criar um novo e-mail no seu app de e-mail com o arquivo anexado.
# machine-drafted, needs native review
palette-share-with-zynk = Compartilhar com o Zynk…
palette-share-with-zynk-help = Criar um link da web para o arquivo no app Zynk.
palette-send-with-zynk = Enviar com o Zynk…
palette-send-with-zynk-help = Enviar o arquivo para um contato ou para um dos seus dispositivos com o Zynk.
palette-export-settings = Exportar configurações…
palette-export-settings-help = Salvar todas as configurações e seus atalhos de teclado em um arquivo.
palette-import-settings = Importar configurações…
palette-import-settings-help = Ler configurações de um arquivo, depois de mostrar o que mudaria.

disabled-in-archive-file = Esta página está dentro de um arquivo compactado e não tem arquivo próprio
disabled-not-rotatable = Este tipo de arquivo não pode registrar uma rotação
disabled-not-on-this-desktop = Seu ambiente de trabalho não oferece isso

## Configurações

settings-browsing-rotate-trim-edges = Aparar bordas de JPEG para girar sem perda
settings-browsing-rotate-trim-edges-help = Um JPEG cujo tamanho não é múltiplo de 8 ou 16 pixels só pode ser girado sem perda cortando os últimos pixels. Sem esta opção, a tag de orientação é alterada, e o corte é oferecido a cada vez.
settings-browsing-rotate-keep-modified = Manter a data de modificação ao girar
settings-browsing-rotate-keep-modified-help = Um arquivo girado mantém a data da última alteração e continua no mesmo lugar quando ordenado por data.
settings-browsing-recent-folders = Pastas recentes
settings-browsing-recent-folders-help = Pastas usadas por último para copiar e mover.

## Janelas

fileop-cancel = Cancelar
rename-title = Renomear
rename-field-label = Nome
rename-button = Renomear
rename-sidecars-follow = Também renomeados: { $files }
rename-extension-changed = Mudar a extensão não converte o arquivo; outros apps podem não abri-lo.
rename-problem-empty = Digite um nome.
rename-problem-invalid-char = Nomes não podem conter “{ $char }”.
rename-problem-reserved = “{ $name }” é um nome que o Windows reserva para dispositivos.
rename-problem-too-long = Esse nome é longo demais; nomes podem ter até 255 bytes.
rename-problem-dot = “.” e “..” não podem ser usados como nomes.
rename-problem-trailing = Nomes não podem terminar com espaço ou ponto.
rename-problem-exists = Outro arquivo já tem este nome.
rename-problem-read-only = Esta pasta não pode ser alterada.
rename-problem-duplicate = Dois arquivos ficariam com este nome.

batch-rename-title =
    { $count ->
        [one] Renomear { $count } arquivo
       *[other] Renomear { $count } arquivos
    }
batch-rename-field-label = Padrão
batch-rename-tokens = Use { "{" }name{ "}" }, { "{" }n:3{ "}" } (001, 002, …), { "{" }date:%Y-%m-%d{ "}" }, { "{" }camera{ "}" }, { "{" }w{ "}" }×{ "{" }h{ "}" } e { "{" }ext{ "}" }.
batch-rename-summary =
    { $count ->
        [one] { $count } arquivo será renomeado
       *[other] { $count } arquivos serão renomeados
    }{ $problems ->
        [0] .
        [one] ; { $problems } não pode e fica como está.
       *[other] ; { $problems } não podem e ficam como estão.
    }
batch-rename-reading = Lendo datas e câmeras…
batch-rename-preview = Prévia
batch-rename-unchanged = mantém o nome
batch-rename-more =
    { $count ->
        [one] Mais { $count } abaixo
       *[other] Mais { $count } abaixo
    }

transfer-copy-title-one = Copiar “{ $file }” para uma pasta
transfer-copy-title =
    { $count ->
        [one] Copiar { $count } arquivo para uma pasta
       *[other] Copiar { $count } arquivos para uma pasta
    }
transfer-move-title-one = Mover “{ $file }” para uma pasta
transfer-move-title =
    { $count ->
        [one] Mover { $count } arquivo para uma pasta
       *[other] Mover { $count } arquivos para uma pasta
    }
transfer-recent = Pastas recentes
transfer-folder = Pasta
transfer-choose-folder = Escolher uma pasta…
transfer-copy-dialog = Copiar para pasta
transfer-move-dialog = Mover para pasta
transfer-sidecars =
    { $count ->
        [one] { $count } arquivo sidecar vai junto.
       *[other] { $count } arquivos sidecar vão junto.
    }
transfer-taken =
    { $count ->
        [one] { $count } nome já está em uso em { $folder }.
       *[other] { $count } nomes já estão em uso em { $folder }.
    }
transfer-when-taken = Quando um nome já está em uso
transfer-skip = Pular
transfer-keep-both = Manter ambos
transfer-replace-option = Substituir
transfer-copy = Copiar
transfer-move = Mover
transfer-replace-title =
    { $count ->
        [one] Substituir { $count } arquivo em { $folder }?
       *[other] Substituir { $count } arquivos em { $folder }?
    }
transfer-replace-note = Os arquivos que estão lá vão para a Lixeira. Desfazer os traz de volta.
transfer-replace = Substituir
transfer-back = Voltar
transfer-stop = Parar

settings-file-import-title = Importar configurações
settings-file-import-dialog = Importar configurações
settings-file-export-dialog = Exportar configurações
settings-file-import-changes = Alterações
settings-file-import-change = { $before } → { $after }
settings-file-import-no-key = sem tecla
settings-file-import-unknown = Não é uma configuração desta versão; ignorada.
settings-file-import-keymap = Atalhos de teclado
settings-file-import-nothing = Nada mudaria: estas configurações já estão em vigor.
settings-file-import-summary =
    { $count ->
        [one] { $count } alteração. A importação pode ser desfeita em Configurações.
       *[other] { $count } alterações. A importação pode ser desfeita em Configurações.
    }
settings-file-import-secrets =
    { $count ->
        [one] { $count } item parece uma senha ou chave e nunca é importado.
       *[other] { $count } itens parecem senhas ou chaves e nunca são importados.
    }
settings-file-import-commands =
    { $count ->
        [one] { $count } comando do usuário não é importado: comandos iniciam programas. Se confiar nele, copie-o você mesmo para o arquivo de configurações.
       *[other] { $count } comandos do usuário não são importados: comandos iniciam programas. Se confiar neles, copie-os você mesmo para o arquivo de configurações.
    }
settings-file-import-button = Importar

## Resultados

fileop-renamed = Renomeado para { $file }
fileop-renamed-count =
    { $count ->
        [one] { $count } arquivo renomeado
       *[other] { $count } arquivos renomeados
    }
fileop-copied =
    { $count ->
        [one] { $count } arquivo copiado para { $folder }
       *[other] { $count } arquivos copiados para { $folder }
    }
fileop-moved =
    { $count ->
        [one] { $count } arquivo movido para { $folder }
       *[other] { $count } arquivos movidos para { $folder }
    }
fileop-skipped =
    { $count ->
        [one] { $count } pulado: o nome já está em uso lá.
       *[other] { $count } pulados: os nomes já estão em uso lá.
    }
fileop-stopped = Parado; o que foi feito continua feito.
fileop-failed-file =
    { $count ->
        [one] Não foi possível copiar ou mover { $file }: { $reason }
       *[other] Não foi possível copiar ou mover { $count } arquivos; o primeiro, { $file }: { $reason }
    }
fileop-progress-copy =
    { $count ->
        [one] Copiando { $count } arquivo… { PERCENT($percent) }
       *[other] Copiando { $count } arquivos… { PERCENT($percent) }
    }
fileop-progress-move =
    { $count ->
        [one] Movendo { $count } arquivo… { PERCENT($percent) }
       *[other] Movendo { $count } arquivos… { PERCENT($percent) }
    }
fileop-done-status = Concluído
fileop-busy = A última alteração de arquivos ainda está em andamento; tente de novo em instantes.
fileop-nothing-to-redo = Nada para refazer
fileop-undone = Desfeito: { $what }
fileop-redone = Refeito: { $what }
fileop-kind-rename = renomeação
fileop-kind-batch-rename = renomeação de vários arquivos
fileop-kind-move = mover para pasta
fileop-kind-copy = copiar para pasta
fileop-kind-rotate = rotação
rename-failed = Não foi possível renomear: { $reason }

undo-problem-changed = Não é possível desfazer: { $file } foi alterado depois, e desfazer perderia essa alteração.
undo-problem-missing = Não é possível desfazer: { $file } não existe mais.
undo-problem-taken = Não é possível desfazer: outro arquivo agora se chama { $file }.
undo-problem-io = Não é possível desfazer a alteração em { $file }: { $reason }

rotate-done-lossless = { $file } girado sem nenhuma perda.
rotate-done-lossless-trimmed = { $file } girado sem nenhuma perda; a borda foi aparada para { $width } × { $height }.
rotate-done-tag = { $file } girado alterando a tag de orientação.
rotate-done-sidecar = Rotação de { $file } salva em { $sidecar }; o arquivo RAW em si nunca é alterado.
rotate-tag-unreadable = Não foi possível ler os dados da imagem o suficiente para girá-la.
rotate-tag-multi-picture = Contém mais de uma imagem, e girar a primeira estragaria as outras.
rotate-tag-trailer = Há dados após a imagem que girar deslocaria.
rotate-tag-not-aligned = Girar os pixels sem perda a apararia para { $width } × { $height }.
rotate-trim-offer = Aparar e girar
rotate-problem-unsupported = { $file } é um tipo de arquivo que não pode registrar uma rotação.
rotate-problem-too-large = { $file } é grande demais para girar.
rotate-problem-damaged = { $file } está danificado onde a orientação é guardada: { $reason }
rotate-problem-io = Não foi possível girar { $file }: { $reason }

share-failed = Não foi possível compartilhar: { $reason }
email-failed = Não foi possível abrir seu app de e-mail: { $reason }
# machine-drafted, needs native review
zynk-opening = Abrindo o Zynk…
zynk-failed = Não foi possível acessar o Zynk: { $reason }
zynk-get-title-send = “Enviar com o Zynk” precisa do app Zynk
zynk-get-title-share = “Compartilhar com o Zynk” precisa do app Zynk
zynk-get-about = O Zynk é um app gratuito para Mac, Windows, Linux, iPhone e Android. Ele envia fotos, vídeos e pastas inteiras direto para seus outros dispositivos e para outras pessoas, de qualquer tamanho. As transferências têm criptografia de ponta a ponta e continuam de onde pararam se a conexão cair. Para um destinatário sem o Zynk, ele cria um link da web para baixar os arquivos pelo navegador.
zynk-get-macos = Baixe o Zynk na Mac App Store. Depois de instalado, o { -app-name } entrega seus arquivos direto para ele.
zynk-get-windows = Baixe o Zynk na Microsoft Store. Depois de instalado, o { -app-name } entrega seus arquivos direto para ele.
zynk-get-linux = O Zynk para Linux é um AppImage, sem nada para instalar: baixe-o, coloque-o em ~/Applications e torne-o executável (nas Propriedades dele ou com chmod +x). O { -app-name } o encontra lá, e em qualquer outro lugar enquanto o Zynk estiver aberto.
zynk-get = Obter o Zynk
zynk-send-old-windows = “Enviar com o Zynk” precisa do Windows 10 versão 2004 ou mais recente. “Compartilhar com o Zynk” funciona neste Windows.
settings-file-export-done = Configurações salvas em { $file }
settings-file-export-failed = Não foi possível salvar as configurações: { $reason }
settings-file-import-unreadable = Não foi possível ler { $file }: { $reason }
settings-file-import-done =
    { $count ->
        [one] { $count } alteração importada
       *[other] { $count } alterações importadas
    }
