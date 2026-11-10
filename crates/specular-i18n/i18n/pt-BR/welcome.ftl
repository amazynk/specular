# machine-drafted, needs native review
# Specular — A tela inicial (quando o Specular abre sem arquivo e quando a última imagem é
# fechada), seus cartões “Para experimentar” e as dicas mostradas uma única vez. Português (Brasil).

## Tela inicial

welcome-title = { -app-name }
welcome-tagline = Um visualizador rápido e fiel para fotos e para imagens técnicas, científicas e médicas.
welcome-open = Abrir um arquivo…
welcome-open-about = Uma imagem, um documento ou um arquivo de dados.
welcome-browse = Navegar em uma pasta…
welcome-browse-about = Escolha uma pasta e veja-a como galeria.
welcome-pictures = Suas imagens
welcome-pictures-about = Todas as fotos da sua biblioteca, por data e local.
welcome-pictures-setup-title = Configurar sua biblioteca…
welcome-pictures-setup = A partir da sua pasta { $folder }.
welcome-pictures-ask = Adicionar sua pasta { $folder } à biblioteca?
welcome-pictures-add = Adicionar pasta
welcome-drop = Você também pode soltar arquivos ou pastas em qualquer lugar desta janela.
welcome-recent = Pastas recentes
welcome-recent-name = { $name }, em { $parent }
welcome-tips = Para experimentar
welcome-tips-hide = Ocultar estas dicas
welcome-tips-hidden = Dicas ocultas. “Mostrar boas-vindas” na paleta de comandos as traz de volta.

## Para experimentar: um nome e uma linha cada. A tecla ao lado do nome é a do usuário.

welcome-tip-gallery = Galeria
welcome-tip-gallery-about = Veja uma pasta inteira de uma vez.
welcome-tip-compare = Comparar
welcome-tip-compare-about = Selecione de duas a quatro imagens na galeria para vê-las lado a lado.
welcome-tip-cull = Modo de triagem
welcome-tip-cull-about = Escolha, rejeite e classifique um ensaio com uma tecla para cada.
welcome-tip-slideshow = Apresentação de slides
welcome-tip-slideshow-about = Mostre uma pasta em tela cheia, uma imagem após a outra.
welcome-tip-palette = Paleta de comandos
welcome-tip-palette-about = Encontre qualquer comando ou configuração pelo nome.
welcome-tip-studies = Estudos
welcome-tip-studies-about = Os pacientes, estudos e séries de um disco ou pasta DICOM.
welcome-tip-dicom-servers = Servidores DICOMweb
welcome-tip-dicom-servers-about = Navegue e abra estudos de um PACS.
welcome-tip-dicom-tags = Tags DICOM
welcome-tip-dicom-tags-about = Todas as tags de uma imagem, pesquisáveis.
welcome-tip-window-level = Janela e nível
welcome-tip-window-level-about = Arraste para ajustar o contraste de uma imagem DICOM.
welcome-tip-omero-servers = Servidores OMERO
welcome-tip-omero-servers-about = Navegue em projetos de microscopia no OMERO e no IDR.
welcome-tip-stretch = Esticamento
welcome-tip-stretch-about = Realce detalhes fracos em dados astronômicos e científicos.
welcome-tip-drawing-layers = Camadas do desenho
welcome-tip-drawing-layers-about = Mostre e oculte as camadas de um desenho técnico.
welcome-tip-needs-image = Abra uma imagem primeiro e depois pressione { $key }.
welcome-tip-needs-gallery = Na galeria, selecione de duas a quatro imagens e depois pressione { $key }.
welcome-tip-key = Atalho: { $key }

## Dicas únicas: cada uma uma vez, discretamente, no máximo uma por início. Configurações ›
## Geral › Mostrar dicas desativa todas.

hint-gallery = Pressione { $key } para ver esta pasta como galeria.
hint-gallery-first = Selecione de duas a quatro imagens e pressione { $compare } para compará-las, ou { $cull } para fazer a triagem da pasta.
hint-raw = Uma pasta de fotos RAW: pressione { $key } para fazer a triagem (escolher, rejeitar e classificar).
hint-pages =
    { $count ->
        [one] Este arquivo tem { $count } página. { $next } e { $prev } mudam de página.
        [many] Este arquivo tem { $count } páginas. { $next } e { $prev } mudam de página.
       *[other] Este arquivo tem { $count } páginas. { $next } e { $prev } mudam de página.
    }
hint-turn-off = Desativar dicas
hint-turned-off = Dicas desativadas. Configurações › Geral › Mostrar dicas as reativa.

## Paleta de comandos

palette-show-welcome = Mostrar boas-vindas
palette-show-welcome-help = A tela inicial, com Abrir, Navegar, suas imagens e coisas para experimentar.
palette-close = Fechar
palette-close-help = Fechar a imagem ou a pasta e voltar à tela inicial. Na tela inicial, sair.

## Primeira execução (abaixo da pergunta do perfil)

profile-on-start = Quando o { -app-name } abre sem arquivo:
profile-skip = Pular
