# Specular — Local AI pack (DESIGN.md "Local AI pack", §10). Portuguese (Portugal). Machine-drafted, needs native review.

## Settings → Local AI
settings-section-ai = IA local
settings-ai-pack = Pacote de IA local
# machine-drafted, needs native review
settings-ai-pack-help = Funcionalidades opcionais que são executadas apenas neste computador. Nada do que encontram sai dele. Escolha um modelo para cada funcionalidade; nada é transferido até o escolher.
# machine-drafted, needs native review
settings-ai-install-from-file = Instalar a partir de ficheiro…
settings-ai-install-from-file-help = Instale ficheiros de modelo que copiou para este computador, para máquinas sem internet. Cada ficheiro é verificado com a lista incluída no { -app-name }.
settings-ai-install-from-folder = Instalar a partir de pasta…
settings-ai-install-from-folder-help = O mesmo, para cada ficheiro de modelo numa pasta.
settings-ai-search = Pesquisa por descrição
settings-ai-search-help = Encontre fotografias descrevendo-as, como «cão numa praia», e agrupe melhor as fotografias semelhantes.
settings-ai-text = Texto em imagens
settings-ai-text-help = Lê o texto de fotografias e digitalizações, para que a pesquisa text: as encontre.
settings-ai-culling = Ajuda na triagem
# machine-drafted, needs native review
settings-ai-culling-help = Assinala olhos fechados, desfocagem e má exposição para ajudar a escolher a melhor fotografia de uma sequência.
settings-ai-people = Pessoas
# machine-drafted, needs native review
settings-ai-people-help = Encontra rostos e agrupa-os por pessoa, para a pesquisa people:. O agrupamento está desativado até o ativar numa biblioteca. Os dados de rostos podem ser eliminados para uma pessoa ou para todas de uma vez.

# The pack's and a feature's state. $percent is formatted ("25%"); $done and $total are
# formatted numbers.
ai-state = { $state ->
        [none] Não há modelos disponíveis nesta versão.
        [installed] Instalado
        [indexing] Instalado · { $done } de { $total } imagens indexadas
        [downloading] A transferir… { $percent }
        [failed] A transferência falhou
       *[available] Não instalado
    }
ai-removing = A remover…
# machine-drafted, needs native review
# Buttons of the Local AI page.
ai-button = { $action ->
        [download] Transferir
        [cancel] Cancelar
        [remove] Remover
        [forget] Esquecer
        [faces] Eliminar todos os dados de rostos
       *[pack] Remover pacote
    }
# The question before deleting.
ai-confirm = { $action ->
        [faces] Eliminar todos os dados de rostos? Todos os rostos e pessoas encontrados são eliminados. As suas fotografias ficam.
       *[pack] Remover o pacote de IA local, os seus modelos e tudo o que encontrou? As suas fotografias não são alteradas.
    }

## Gallery search
gallery-query-needs-ai = { $field }: precisa do pacote de IA local (Definições › IA local)
# machine-drafted, needs native review
gallery-query-ai-not-yet = { $field }: o pacote de IA local ainda não consegue pesquisar isto

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
cull-flag-dark = Demasiado escura
# machine-drafted, needs native review
cull-flag-bright = Demasiado clara
# machine-drafted, needs native review
cull-flag-clipped = Altas luzes ou sombras cortadas
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
people-show-photos = Mostrar fotografias
# machine-drafted, needs native review
people-name = Atribuir nome…
# machine-drafted, needs native review
people-name-prompt = Atribuir um nome a esta pessoa
# machine-drafted, needs native review
people-faces-open = Rostos
# machine-drafted, needs native review
people-faces-of = Rostos de { $name }
# machine-drafted, needs native review
people-merge = Unir
# machine-drafted, needs native review
people-merged = { $count ->
        [one] { $count } pessoa unida
       *[other] { $count } pessoas unidas
    }
# machine-drafted, needs native review
people-merge-ask = Unir { $from } a { $into }?
# machine-drafted, needs native review
people-merge-ask-many = Juntar { $count } pessoas em { $into }?
# machine-drafted, needs native review
people-merge-ask-detail = Os rostos passam a ser de { $into }. As fotografias no disco não mudam.
# machine-drafted, needs native review
people-not-this = Não é esta pessoa
# machine-drafted, needs native review
people-face-removed = Rosto retirado desta pessoa. A fotografia mantém-se.
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
people-loading = A carregar…
# machine-drafted, needs native review
people-not-installed = Pessoas não está instalado. Transfira-o em Definições › IA local.
# machine-drafted, needs native review
people-grouping-off = O agrupamento de rostos está desativado nesta biblioteca. Ative «Agrupar rostos em pessoas» em Definições › IA local.
# machine-drafted, needs native review
people-none = Ainda não foram encontradas pessoas.

# machine-drafted, needs native review
# A feature's model: Default (the model Specular ships as the default, or the nearest one
# this computer can run) or a model by name, which stays that model on any computer. The
# page shows it only for a feature with more than one model; the dropdown labels Default
# with the model it means (ai-size-recommended-option) and names each model as the model
# list does. The option names below are the palette's and the command line's: model names
# are proper names, kept as they are.
settings-ai-search-size = Modelo
settings-ai-search-size-help = SigLIP 2 B/16 encontra melhor as fotografias, em muitos idiomas. B/32 indexa cerca de quatro vezes mais depressa, para placas gráficas mais lentas.
settings-ai-search-size-recommended = Predefinido
# machine-drafted, needs native review
settings-ai-search-size-small = SigLIP 2 B/32
settings-ai-search-size-standard = PE Core S16
settings-ai-search-size-large = SigLIP 2 B/16
settings-ai-text-size = Modelo
settings-ai-text-size-help = PP-OCRv6 small lê com mais precisão; tiny é mais rápido.
settings-ai-text-size-recommended = Predefinido
settings-ai-text-size-small = PP-OCRv6 tiny
settings-ai-text-size-standard = PP-OCRv6 small
settings-ai-text-size-large = Grande
# machine-drafted, needs native review
settings-ai-culling-size = Modelo
settings-ai-culling-size-help = O modelo que deteta olhos fechados.
settings-ai-culling-size-recommended = Predefinido
settings-ai-culling-size-small = Pequeno
# machine-drafted, needs native review
settings-ai-culling-size-standard = Face Landmarker
settings-ai-culling-size-large = Grande
# machine-drafted, needs native review
settings-ai-people-size = Modelo
settings-ai-people-size-help = SFace é pequeno. AuraFace é maior, treinado com dados que o editor diz estarem licenciados para uso comercial.
settings-ai-people-size-recommended = Predefinido
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
ai-size-recommended-option = Predefinido ({ $size })
# Under the model setting: the default, or the nearest model this computer can run. $gpu is
# the graphics card's name.
ai-recommended-for = Predefinido para a sua { $gpu }: { $size }
ai-recommended = Predefinido para este computador: { $size }
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
        [setting] Executa em: CPU (o processamento está definido como «Apenas a CPU»)
        [checking] Executa em: CPU — a verificar a GPU em relação a ela
       *[cpu] Executa em: CPU
    }
# A size's row: its files on disk ($place "disk"), or what it would download, the memory it
# needs, and " · needs the GPU" only for a size that cannot run without one. $bytes and
# $memory are formatted sizes.
ai-size-detail = { $place ->
        [disk] { $bytes } em disco
       *[download] { $bytes } a transferir
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
        [gpu] Precisa de uma GPU e este computador não tem nenhuma que possa utilizar.
       *[memory] Precisa de { $need } de memória, mais do que os { $have } que o Specular permite para IA neste computador.
    }
# Over the size setting while it names a size other than the one installed and in use
# ($current, $chosen: ai-tier). The rows below have the buttons to change it.
ai-size-not-in-use = { $current } está em utilização, não { $chosen }.
# machine-drafted, needs native review
# Buttons of a size's row and of the size question: "use" chooses that size (one not
# downloaded; a downloaded one says Switch, ai-size-button), "back" returns the setting to
# Recommended, "anyway" keeps a size that may not run on this computer (ai-unfit-question).
# The screen reader adds the size's name to each.
ai-size-action = { $action ->
        [back] Voltar ao predefinido
        [anyway] Utilizar mesmo assim
       *[use] Utilizar este tamanho
    }
# The size the setting chose is not installed while another one is ($current, ai-tier).
ai-size-chosen-missing = Escolhido, mas ainda não transferido: o tamanho { $current } continua em utilização.
# What a size is now: from what is installed, never from the setting. "downloaded": its files
# are here but another size is in use; "missing": the chosen size, not downloaded.
ai-size-state = { $state ->
        [current] Em utilização
        [downloaded] Transferido, fora de utilização
        [missing] Não transferido
        [previous] Em utilização até o novo tamanho indexar a sua biblioteca
        [target] A transferir…
        [failed] A transferência falhou
       *[other] { "" }
    }
# A licence row's button (opens the licence's page).
ai-licence = Ver licença
# Asked before a size change that downloads and re-indexes. $feature is the feature's name,
# $size the new size (ai-tier), $download a formatted size.
ai-switch-question = { $download ->
        [none] { $feature }: mudar para { $size }? A sua biblioteca será indexada novamente; até lá, o tamanho atual continua em utilização.
       *[other] { $feature }: mudar para { $size }? Isto transfere { $download } e indexa novamente a sua biblioteca; até lá, o tamanho atual continua em utilização.
    }
# machine-drafted, needs native review
# A size that may not run on this computer, chosen for a feature not installed yet: asked
# before the setting keeps it (ai-size-unfit says why, on the next line). $size is ai-tier,
# $feature the feature's name.
ai-unfit-question = { $feature }: utilizar { $size } mesmo assim? Pode não funcionar neste computador, ou apenas muito lentamente.
# The recommendation moved: offered, never applied silently.
ai-suggest-question = { $feature }: { $size } é agora mais adequado para este computador. Mudar? { $download ->
        [none] A sua biblioteca será indexada novamente.
       *[other] Isto transfere { $download } e indexa novamente a sua biblioteca.
    }
ai-suggest-toast = IA local: outro tamanho de modelo é mais adequado para este computador (Definições › IA local).
# A feature switching size: the new size ($size, ai-tier) indexes the library while the old one
# stays in use. $done and $total are formatted numbers.
ai-switching = A mudar para { $size } · { $done } de { $total } imagens indexadas

## The model manager: what each model says besides its size and licence

# machine-drafted, needs native review
# A model or feature not installable yet.
ai-coming = Disponível numa versão futura
# machine-drafted, needs native review
# What a model reads. $languages: multilingual, english, or a script id (OCR packs).
ai-choice-languages = { $languages ->
        [multilingual] Muitos idiomas
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
        [latin] Alfabeto latino (muitas línguas europeias)
       *[other] { $languages }
    }
# A hint about a model. $hint: faster, closed-eyes, small-faces.
ai-choice-hint = { $hint ->
        [faster] Mais rápido, para placas gráficas mais lentas
        [closed-eyes] Deteta olhos fechados
        [small-faces] Melhor com rostos pequenos
        [no-translate] Não traduz para inglês
       *[other] { "" }
    }
# A model's training data. $source: the data as its publisher names it ($named is "no" when
# it is not named). $terms: whether the publisher states the data's terms ("stated"), does
# not ("unstated"), or the data is licensed for research only ("research").
ai-choice-data = { $terms ->
        [stated] Dados de treino: { $source }
        [research] Dados de treino: { $source } (licenciados apenas para investigação)
       *[unstated] { $named ->
            [no] Dados de treino: não indicados, termos não declarados
           *[yes] Dados de treino: { $source }, termos não declarados
        }
    }
# Where a model downloads from. $host is a host name ("huggingface.co").
ai-choice-host = Transferido de { $host }
# A model still coming, in a feature's row. $size is formatted, $licence an SPDX name.
ai-choice-coming = { $name }: { $size }, { $licence }
# People: grouping faces into people, per library.
ai-grouping = Agrupar rostos em pessoas
ai-grouping-help = Desativado até o ativar nesta biblioteca. Desativá-lo interrompe o agrupamento; os rostos já agrupados ficam até os eliminar.
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
ai-addon-first = Transfira primeiro { $feature }.
# machine-drafted, needs native review
# Asked before an add-on is removed. $name is the model's name ("PP-OCRv5 Arabic").
ai-addon-remove-title = Remover { $name }?
# machine-drafted, needs native review
ai-addon-remove-detail = Os ficheiros do modelo são eliminados. As suas fotografias e o texto já encontrado ficam.
# machine-drafted, needs native review
# Text: reading the text in this library's photos, per library.
ai-text-indexing = Ler o texto das fotografias desta biblioteca
# machine-drafted, needs native review
ai-text-indexing-help = Desativado até o ativar nesta biblioteca. Depois, text: na pesquisa da Biblioteca encontra fotografias pelas palavras que contêm.
# machine-drafted, needs native review
ai-text-indexing-state = { $on ->
        [yes] Ativado nesta biblioteca
       *[no] Desativado
    }

## Instalar a partir de ficheiro

# machine-drafted, needs native review
# After model files were copied in.
ai-sideload-ready = { $count ->
        [one] { $count } ficheiro de modelo copiado. Prima «Transferir» para o instalar.
        [many] { $count } ficheiros de modelo copiados. Prima «Transferir» para os instalar.
       *[other] { $count } ficheiros de modelo copiados. Prima «Transferir» para os instalar.
    }
ai-sideload-none = Nenhum ficheiro de modelo encontrado.
# $file is a file name, or several separated by commas.
ai-sideload-unknown = { $file } não é um modelo que o { -app-name } conheça. Não foi copiado.
ai-sideload-failed = Não foi possível copiar { $file }: { $reason }
# Asked before using a file whose SHA-256 isn't in the shipped list. $model is the model it
# would be used as ("clip-vit-b32-1").
ai-unverified-title = Utilizar um modelo não verificado?
ai-unverified-detail = { $file } não está na lista de modelos incluídos no { -app-name }, por isso não pode ser verificado. Será utilizado como { $model }.
ai-unverified-use = Utilizar modelo não verificado

# machine-drafted, needs native review
ai-failed-internal = Algo correu mal no { -app-name }. O registo tem os detalhes.
# The buttons under a model-size question: switch, or keep the size in use.
ai-size-button = { $action ->
        [switch] Mudar
       *[keep] Manter o tamanho atual
    }
