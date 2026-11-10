# machine-drafted, needs native review
# Specular — O ecrã inicial (quando o Specular abre sem ficheiro e quando a última imagem é
# fechada), os seus cartões «Para experimentar» e as sugestões mostradas uma única vez. Português (Portugal).

## Ecrã inicial

welcome-title = { -app-name }
welcome-tagline = Um visualizador rápido e fiel para fotografias e para imagens técnicas, científicas e médicas.
welcome-open = Abrir um ficheiro…
welcome-open-about = Uma imagem, um documento ou um ficheiro de dados.
welcome-browse = Percorrer uma pasta…
welcome-browse-about = Escolha uma pasta e veja-a como galeria.
welcome-pictures = As suas imagens
welcome-pictures-about = Todas as fotografias da sua biblioteca, por data e local.
welcome-pictures-setup-title = Configurar a sua biblioteca…
welcome-pictures-setup = A partir da sua pasta { $folder }.
welcome-pictures-ask = Adicionar a sua pasta { $folder } à biblioteca?
welcome-pictures-add = Adicionar pasta
welcome-drop = Também pode largar ficheiros ou pastas em qualquer ponto desta janela.
welcome-recent = Pastas recentes
welcome-recent-name = { $name }, em { $parent }
welcome-tips = Para experimentar
welcome-tips-hide = Ocultar estas sugestões
welcome-tips-hidden = Sugestões ocultas. «Mostrar boas-vindas» na paleta de comandos volta a mostrá-las.

## Para experimentar: um nome e uma linha cada. A tecla ao lado do nome é a do utilizador.

welcome-tip-gallery = Galeria
welcome-tip-gallery-about = Veja uma pasta inteira de uma só vez.
welcome-tip-compare = Comparar
welcome-tip-compare-about = Selecione duas a quatro imagens na galeria para as ver lado a lado.
welcome-tip-cull = Modo de triagem
welcome-tip-cull-about = Escolha, rejeite e classifique uma sessão com uma tecla para cada.
welcome-tip-slideshow = Apresentação de diapositivos
welcome-tip-slideshow-about = Mostre uma pasta em ecrã inteiro, uma imagem após outra.
welcome-tip-palette = Paleta de comandos
welcome-tip-palette-about = Encontre qualquer comando ou definição pelo nome.
welcome-tip-studies = Estudos
welcome-tip-studies-about = Os pacientes, estudos e séries de um disco ou pasta DICOM.
welcome-tip-dicom-servers = Servidores DICOMweb
welcome-tip-dicom-servers-about = Percorra e abra estudos de um PACS.
welcome-tip-dicom-tags = Etiquetas DICOM
welcome-tip-dicom-tags-about = Todas as etiquetas de uma imagem, pesquisáveis.
welcome-tip-window-level = Janela e nível
welcome-tip-window-level-about = Arraste para ajustar o contraste de uma imagem DICOM.
welcome-tip-omero-servers = Servidores OMERO
welcome-tip-omero-servers-about = Percorra projetos de microscopia no OMERO e no IDR.
welcome-tip-stretch = Esticamento
welcome-tip-stretch-about = Realce detalhes ténues em dados astronómicos e científicos.
welcome-tip-drawing-layers = Camadas do desenho
welcome-tip-drawing-layers-about = Mostre e oculte as camadas de um desenho técnico.
welcome-tip-needs-image = Abra primeiro uma imagem e depois prima { $key }.
welcome-tip-needs-gallery = Na galeria, selecione duas a quatro imagens e depois prima { $key }.
welcome-tip-key = Atalho: { $key }

## Sugestões únicas: cada uma uma vez, discretamente, no máximo uma por arranque. Definições ›
## Geral › Mostrar sugestões desativa-as todas.

hint-gallery = Prima { $key } para ver esta pasta como galeria.
hint-gallery-first = Selecione duas a quatro imagens e prima { $compare } para as comparar, ou { $cull } para fazer a triagem da pasta.
hint-raw = Uma pasta de fotografias RAW: prima { $key } para fazer a triagem (escolher, rejeitar e classificar).
hint-pages =
    { $count ->
        [one] Este ficheiro tem { $count } página. { $next } e { $prev } mudam de página.
        [many] Este ficheiro tem { $count } páginas. { $next } e { $prev } mudam de página.
       *[other] Este ficheiro tem { $count } páginas. { $next } e { $prev } mudam de página.
    }
hint-turn-off = Desativar sugestões
hint-turned-off = Sugestões desativadas. Definições › Geral › Mostrar sugestões volta a ativá-las.

## Paleta de comandos

palette-show-welcome = Mostrar boas-vindas
palette-show-welcome-help = O ecrã inicial, com Abrir, Percorrer, as suas imagens e coisas para experimentar.
palette-close = Fechar
palette-close-help = Fechar a imagem ou a pasta e voltar ao ecrã inicial. No ecrã inicial, sair.

## Primeira execução (abaixo da pergunta do perfil)

profile-on-start = Quando o { -app-name } abre sem ficheiro:
profile-skip = Ignorar
