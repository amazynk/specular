# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Portuguese (Brazil). Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = IA local
settings-ai-pack = Pacote de IA local
# machine-drafted, needs native review
settings-ai-pack-help = Recursos opcionais que rodam apenas neste computador. Nada do que encontram sai dele. Escolha um modelo para cada recurso; nada é baixado até você escolher.
# machine-drafted, needs native review
settings-ai-install-from-file = Instalar de arquivo…
settings-ai-install-from-file-help = Instale arquivos de modelo que você copiou para este computador, para máquinas sem internet. Cada arquivo é conferido com a lista que acompanha o { -app-name }.
settings-ai-install-from-folder = Instalar de pasta…
settings-ai-install-from-folder-help = O mesmo, para cada arquivo de modelo de uma pasta.
settings-ai-search = Pesquisa por descrição
settings-ai-search-help = Encontre fotos descrevendo-as, como “cachorro na praia”, e agrupe melhor fotos parecidas.
settings-ai-text = Texto em imagens
settings-ai-text-help = Lê o texto de fotos e digitalizações para que a pesquisa text: as encontre.
settings-ai-culling = Ajuda na triagem
# machine-drafted, needs native review
settings-ai-culling-help = Sinaliza olhos fechados, desfoque e exposição ruim para ajudar a escolher a melhor foto de uma sequência.
settings-ai-people = Pessoas
# machine-drafted, needs native review
settings-ai-people-help = Encontra rostos e os agrupa por pessoa, para a pesquisa people:. O agrupamento fica desligado até você ativá-lo em uma biblioteca. Os dados de rostos podem ser excluídos de uma pessoa ou de todas de uma vez.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] Nenhum modelo disponível nesta versão.
        [installed] Instalado
        [indexing] Instalado · { $done } de { $total } imagens indexadas
        [downloading] Baixando… { $percent }
        [failed] Falha no download
       *[available] Não instalado
    }
ai-removing = Removendo…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Baixar
        [cancel] Cancelar
        [remove] Remover
        [forget] Esquecer
        [faces] Excluir todos os dados de rostos
       *[pack] Remover pacote
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Excluir todos os dados de rostos? Todos os rostos e pessoas encontrados são excluídos. Suas fotos permanecem.
       *[pack] Remover o pacote de IA local, seus modelos e tudo o que ele encontrou? Suas fotos não são alteradas.
    }

## Gallery search
gallery-query-needs-ai = { $field }: precisa do pacote de IA local (Configurações › IA local)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: o pacote de IA local ainda não consegue pesquisar isso

## Faces: People view, face boxes and culling flags
# machine-drafted, needs native review
# "Show faces" on the picture.
faces-shown = Rostos visíveis
# machine-drafted, needs native review
faces-hidden = Rostos ocultos
# machine-drafted, needs native review
# Beside a face box whose eyes look closed.
faces-eyes-closed = Olhos fechados
# machine-drafted, needs native review
# Culling flags on the cull mode strip.
cull-flag-eyes-closed = Olhos fechados
# machine-drafted, needs native review
cull-flag-eyes-closed-some = Olhos fechados: { $closed } de { $faces } rostos
# machine-drafted, needs native review
cull-flag-blurred = Desfocada
# machine-drafted, needs native review
cull-flag-dark = Escura demais
# machine-drafted, needs native review
cull-flag-bright = Clara demais
# machine-drafted, needs native review
cull-flag-clipped = Realces ou sombras recortados
# machine-drafted, needs native review
# The People view.
people-title = Pessoas
# machine-drafted, needs native review
people-open = Pessoas
# machine-drafted, needs native review
people-done = Concluído
# machine-drafted, needs native review
people-back = Voltar
# machine-drafted, needs native review
people-show-photos = Mostrar fotos
# machine-drafted, needs native review
people-name = Nomear…
# machine-drafted, needs native review
people-name-prompt = Dar nome a esta pessoa
# machine-drafted, needs native review
people-faces-open = Rostos
# machine-drafted, needs native review
people-faces-of = Rostos de { $name }
# machine-drafted, needs native review
people-merge = Mesclar
# machine-drafted, needs native review
people-merged = { $count ->
        [one] { $count } pessoa mesclada
       *[other] { $count } pessoas mescladas
    }
# machine-drafted, needs native review
people-merge-ask = Mesclar { $from } com { $into }?
# machine-drafted, needs native review
people-merge-ask-many = Mesclar { $count } pessoas em { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = Os rostos passam a ser de { $into }. As fotos no disco não mudam.
# machine-drafted, needs native review
people-not-this = Não é esta pessoa
# machine-drafted, needs native review
people-face-removed = Rosto removido desta pessoa. A foto continua.
# machine-drafted, needs native review
# Someone not named yet.
people-unnamed = Pessoa #{ $id }
# machine-drafted, needs native review
people-face-count = { $count ->
        [one] { $count } rosto
       *[other] { $count } rostos
    }
# machine-drafted, needs native review
people-count = { $count ->
        [one] { $count } pessoa
       *[other] { $count } pessoas
    }
# machine-drafted, needs native review
people-face-a11y = Rosto { $index }
# machine-drafted, needs native review
people-loading = Carregando…
# machine-drafted, needs native review
people-not-installed = Pessoas não está instalado. Baixe em Configurações › IA local.
# machine-drafted, needs native review
people-grouping-off = O agrupamento de rostos está desligado nesta biblioteca. Ative “Agrupar rostos em pessoas” em Configurações › IA local.
# machine-drafted, needs native review
people-none = Nenhuma pessoa encontrada ainda.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Modelo
settings-ai-search-size-help = SigLIP 2 B/16 encontra fotos melhor, em vários idiomas. B/32 indexa cerca de quatro vezes mais rápido, para placas de vídeo mais lentas.
settings-ai-search-size-recommended = Padrão
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Modelo
settings-ai-text-size-help = PP-OCRv6 small lê com mais precisão; tiny é mais rápido.
settings-ai-text-size-recommended = Padrão
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Grande
# machine-drafted, needs native review
settings-ai-culling-size = Modelo
settings-ai-culling-size-help = O modelo que detecta olhos fechados.
settings-ai-culling-size-recommended = Padrão
settings-ai-culling-size-small = Pequeno
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Grande
# machine-drafted, needs native review
settings-ai-people-size = Modelo
settings-ai-people-size-help = SFace é pequeno. AuraFace é maior, treinado com dados que o publicador diz serem licenciados para uso comercial.
settings-ai-people-size-recommended = Padrão
settings-ai-people-size-small = Pequeno
# machine-drafted, needs native review
settings-ai-people-size-standard = BlazeFace + SFace
settings-ai-people-size-large = BlazeFace + AuraFace
# A model size by name.
ai-tier = { $tier ->
        [small] Pequeno
        [large] Grande
       *[standard] Padrão
    }
# machine-drafted, needs native review
# Default in the model dropdown, and the label of the "Back to default" row: with the model
# it means on this computer, "Default (SigLIP 2 B/16)". $size is the model's name.
ai-size-recommended-option = Padrão ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Padrão para sua { $gpu }: { $size }
ai-recommended = Padrão para este computador: { $size }
# Nothing fits: no size can run here.
ai-recommended-none = Nenhum tamanho cabe na memória deste computador.
# Under an installed feature: what its model runs on now. $gpu is the graphics card's name
# (used with "gpu" only). The CPU variants say why the graphics card is not used.
ai-runs-on = { $engine ->
        [gpu] Executa em: GPU ({ $gpu })
        [gpu-unnamed] Executa em: GPU
        [no-gpu] Executa em: CPU (nenhuma GPU encontrada)
        [differed] Executa em: CPU — o resultado da GPU diferiu do da CPU
        [failed] Executa em: CPU — o modelo falhou na GPU
        [setting] Executa em: CPU (o processamento está em “Somente a CPU”)
        [checking] Executa em: CPU — verificando a GPU em relação a ela
       *[cpu] Executa em: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } em disco
       *[download] { $bytes } para baixar
    } · { $memory } de memória{ $gpu ->
        [yes] { " · precisa da GPU" }
       *[no] { "" }
    }
# machine-drafted, needs native review
# Under a size that may not run on this computer (it is still shown, and choosing it asks
# first). "gpu": it needs a graphics card this computer does not have or cannot use;
# "memory": $need is the memory it needs, $have what this computer has for it (formatted
# sizes, "6 GB").
ai-size-unfit = { $why ->
        [gpu] Precisa de uma GPU, e este computador não tem nenhuma que possa usar.
       *[memory] Precisa de { $need } de memória, mais que os { $have } que o Specular permite para IA neste computador.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = { $current } está em uso, não { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Voltar ao padrão
        [anyway] Usar mesmo assim
       *[use] Usar este tamanho
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Escolhido, mas ainda não baixado: o tamanho { $current } continua em uso.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] Em uso
        [downloaded] Baixado, fora de uso
        [missing] Não baixado
        [previous] Em uso até o novo tamanho indexar sua biblioteca
        [target] Baixando…
        [failed] Falha no download
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Ver licença
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: mudar para { $size }? Sua biblioteca será indexada novamente; o tamanho atual continua em uso até lá.
       *[other] { $feature }: mudar para { $size }? Isso baixa { $download } e indexa sua biblioteca novamente; o tamanho atual continua em uso até lá.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: usar { $size } mesmo assim? Pode não funcionar neste computador, ou funcionar só muito devagar.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: { $size } agora é mais adequado para este computador. Mudar? { $download ->
        [none] Sua biblioteca será indexada novamente.
       *[other] Isso baixa { $download } e indexa sua biblioteca novamente.
    }
ai-suggest-toast = IA local: outro tamanho de modelo é mais adequado para este computador (Configurações › IA local).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = Mudando para { $size } · { $done } de { $total } imagens indexadas

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Disponível em uma versão futura
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Vários idiomas
        [english] Apenas inglês
        [cjk-latin] Chinês, japonês e alfabeto latino
        [zh-latin] Chinês e alfabeto latino
        [arabic] Árabe
        [hebrew] Hebraico
        [korean] Coreano
        [cyrillic] Cirílico
        [greek] Grego
        [thai] Tailandês
        [devanagari] Devanágari
        [tamil] Tâmil
        [telugu] Telugu
        [eslav] Russo, ucraniano e bielorrusso
        [latin] Alfabeto latino (muitos idiomas europeus)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Mais rápido, para placas de vídeo mais lentas
        [closed-eyes] Detecta olhos fechados
        [small-faces] Melhor com rostos pequenos
        [no-translate] Não traduz para inglês
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Dados de treinamento: { $source }
        [research] Dados de treinamento: { $source } (licenciados só para pesquisa)
       *[unstated] { $named ->
            [no] Dados de treinamento: não informados, termos não declarados
           *[yes] Dados de treinamento: { $source }, termos não declarados
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Baixado de { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Agrupar rostos em pessoas
ai-grouping-help = Desligado até você ativá-lo nesta biblioteca. Desativar interrompe o agrupamento; rostos já agrupados ficam até você excluí-los.
ai-grouping-state = { $on ->
        [yes] Ativado nesta biblioteca
       *[no] Desativado
    }
ai-grouping-button = { $on ->
        [yes] Desativar
       *[no] Ativar
    }
# machine-drafted, needs native review
# An add-on (a script's text recogniser) before its feature is downloaded. $feature is the
# feature's name ("Text in photos").
ai-addon-first = Baixe { $feature } primeiro.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Remover { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = Os arquivos do modelo são excluídos. Suas fotos e o texto já encontrado permanecem.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Ler o texto das fotos desta biblioteca
# machine-drafted, needs native review
ai-text-indexing-help = Desligado até você ativá-lo nesta biblioteca. Depois, text: na pesquisa da Biblioteca encontra fotos pelas palavras que elas contêm.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Ativado nesta biblioteca
       *[no] Desativado
    }

## Instalar de arquivo

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] { $count } arquivo de modelo copiado. Clique em “Baixar” para instalá-lo.
        [many] { $count } arquivos de modelo copiados. Clique em “Baixar” para instalá-los.
       *[other] { $count } arquivos de modelo copiados. Clique em “Baixar” para instalá-los.
    }
ai-sideload-none = Nenhum arquivo de modelo encontrado.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } não é um modelo que o { -app-name } conhece. Ele não foi copiado.
ai-sideload-failed = Não foi possível copiar { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Usar um modelo não verificado?
ai-unverified-detail = { $file } não está na lista de modelos que acompanha o { -app-name }, então não pode ser conferido. Ele será usado como { $model }.
ai-unverified-use = Usar modelo não verificado

# machine-drafted, needs native review
ai-failed-internal = Algo deu errado dentro do { -app-name }. O log tem os detalhes.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Mudar
       *[keep] Manter o tamanho atual
    }
