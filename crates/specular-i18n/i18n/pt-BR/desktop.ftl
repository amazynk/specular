# machine-drafted, needs native review
# Integração profunda com a área de trabalho: Configurações → Integração, definir como papel de parede. Português do Brasil.

settings-section-integration = Integração
settings-integration-default-viewer = Visualizador de imagens padrão
settings-integration-default-viewer-help = Abrir imagens com o { -app-name } ao clicar duas vezes nelas. Remover restaura o visualizador usado antes.
# machine-drafted, needs native review
settings-integration-extra-default-types = Mais tipos de arquivo
settings-integration-extra-default-types-help = Outros tipos de arquivo para abrir com o { -app-name } ao clicar duas vezes, escolhidos em Visualizador de imagens padrão.
settings-integration-thumbnailer = Miniaturas no gerenciador de arquivos
settings-integration-thumbnailer-help = Permite que gerenciadores de arquivos mostrem prévias de formatos que eles mesmos não leem, como JPEG XL, HEIC e arquivos RAW de câmera.
settings-integration-single-instance = Instância única via D-Bus
settings-integration-single-instance-help = Arquivos abertos pelo gerenciador de arquivos, por um lançador ou pelo terminal vão para a janela do { -app-name } em execução. Vale para janelas abertas depois.
settings-integration-file-manager-actions = Ações no gerenciador de arquivos
settings-integration-file-manager-actions-help = Adiciona “Abrir”, “Comparar” e “Definir como papel de parede” aos menus de contexto do Nautilus, Dolphin, Thunar e Nemo.
settings-integration-search-provider = Pesquisa da área de trabalho
# machine-drafted, needs native review
settings-integration-search-provider-help = Encontre imagens da sua biblioteca na pesquisa do GNOME e do KDE. As palavras funcionam como na barra de pesquisa da biblioteca.
# machine-drafted, needs native review
settings-integration-url-scheme = Abrir links specular://
settings-integration-url-scheme-help = Links como specular:///home/me/photo.jpg em um navegador ou outro programa abrem no { -app-name }. Somente arquivos deste computador são abertos, e os incomuns só depois que você confirmar.

integration-state-on = Ativado
integration-state-off = Desativado
integration-state-partial = Configurado em parte. Aplique de novo para reparar.
integration-state-unavailable = Indisponível nesta área de trabalho
integration-state-sandboxed = Indisponível dentro de um sandbox (Flatpak)
integration-state-no-bus = Indisponível: não há sessão D-Bus
integration-state-no-file-manager = Indisponível: nenhum gerenciador de arquivos compatível foi encontrado
integration-state-no-search = Indisponível: nem o GNOME Shell nem o KRunner foram encontrados
integration-state-working = Trabalhando…
integration-state-details = { $state } · { $details }
integration-detail-types = tipos de imagem: { $count } de { $total }
integration-detail-thumbnail-types = tipos de imagem: { $count }
integration-apply = Aplicar
integration-remove = Remover
integration-done-on = “{ $option }” ativado
integration-done-off = “{ $option }” desativado
integration-failed = Não foi possível alterar “{ $option }”: { $reason }
# machine-drafted, needs native review
integration-search-remove-ask = Desativar a pesquisa da área de trabalho?
integration-search-remove-detail = O { -app-name } é retirado da pesquisa do { $desktops }. Sua biblioteca e suas imagens continuam.
search-provider-open-folder = Abrir pasta

integration-label-open = Abrir no { -app-name }
integration-label-compare = Comparar no { -app-name }
integration-label-wallpaper = Definir como papel de parede
integration-label-generic-name = Visualizador de imagens
integration-label-comment = Abra e folheie fotos grandes instantaneamente

palette-set-wallpaper = Definir como papel de parede
wallpaper-set = Papel de parede definido ({ $method })
wallpaper-failed = Não foi possível definir o papel de parede: { $reason }

# machine-drafted, needs native review
url-invalid = Não é possível abrir este link do { -app-name }: { $reason }
url-not-found = Os arquivos indicados por este link não existem
url-remote-refused = O { -app-name } abre apenas arquivos deste computador, não { $source }
url-confirm-files = { $more ->
    [0] “{ $file }”
   *[other] “{ $file }” e mais { $more }
}
url-confirm-title = { $reason ->
    [dots] Abrir { url-confirm-files }? O link chega até ele por pastas “..”.
    [network] Abrir { url-confirm-files }? Ele está em um compartilhamento de rede ou dispositivo.
   *[hidden] Abrir { url-confirm-files }? O nome contém caracteres invisíveis ou de direção de texto.
}
url-confirm-open = Abrir

# Launcher actions and recent files: machine-drafted, needs native review
integration-label-new-window = Nova janela
integration-label-library = Biblioteca
integration-label-slideshow = Apresentação de Imagens
settings-integration-recent-files = Arquivos recentes da área de trabalho
settings-integration-recent-files-help = As imagens abertas aparecem em “Recentes” no gerenciador de arquivos e nas listas de arquivos recentes da área de trabalho. Ao remover, as entradas do { -app-name } saem dessas listas.
integration-detail-recent = arquivos: { $count }
# machine-drafted, needs native review
integration-extra-title = Mais tipos de arquivo
# machine-drafted, needs native review
integration-extra-intro = Além de imagens, o { -app-name } pode abrir estes arquivos ao clicar duas vezes. Nada fica marcado até você marcar. Os programas que editam esses arquivos continuam em “Abrir com”, e desmarcar um tipo o devolve ao programa que tinha antes. Blackmagic RAW, DWG, STEP, IGES e Zeiss CZI também precisam de software gratuito dos seus fabricantes: veja Configurações › Decodificadores opcionais.
integration-extra-chosen = { $count ->
    [0] Nenhum escolhido
    [one] 1 tipo escolhido
    [many] { $count } de tipos escolhidos
   *[other] { $count } tipos escolhidos
}
integration-extra-count = { $on } de { $total }
integration-extra-none-yet = Nenhum ainda
integration-extra-keywords = tipos de arquivo formatos associações clique duplo abrir com editor ciência médico CAD SIG GIS 3D moléculas vídeo SVG PDF
integration-extra-group-editors = Arquivos de editores
# machine-drafted, needs native review
integration-extra-group-editors-help = Documentos de programas de pintura e desenho: Krita, GIMP, Paint.NET, PaintShop Pro, Procreate, Affinity, OpenRaster, Illustrator, CorelDRAW, Clip Studio Paint, Sketch e Figma. O clique duplo mostra a imagem; a edição continua no programa que a criou.
integration-extra-group-science = Ciência
integration-extra-group-science-help = Arquivos de instrumentos e software de pesquisa: câmeras térmicas, microscópios, medições de superfície, HDF5, NetCDF, dados sísmicos e meteorológicos.
integration-extra-group-medical = Médico
integration-extra-group-medical-help = Exames médicos e imagens do cérebro: DICOM, NIfTI, MINC, NRRD, tractografia e superfícies cerebrais.
integration-extra-group-cad-gis = CAD e SIG
# machine-drafted, needs native review
integration-extra-group-cad-gis-help = Desenhos de plotter, ilustrações técnicas, layouts de chips e mapas: HP-GL, CGM, GDSII, shapefiles, GeoPackage, KML, GPX, GeoJSON e blocos de elevação. DWG abre com o ODA File Converter, gratuito, e STEP e IGES com o OpenCASCADE.
integration-extra-group-models = 3D e moléculas
integration-extra-group-models-help = Modelos 3D, modelos de edifícios, nuvens de pontos e moléculas: STL, OBJ, glTF, PLY, 3MF, X3D, IFC, LAS, E57, PDB, CIF e mais.
integration-extra-group-videos = Vídeos
# machine-drafted, needs native review
integration-extra-group-videos-help = Vídeos: MP4, MOV, M4V, 3GP, 3G2, MKV, WebM, AVCHD (MTS, M2TS), OGV, IVF e MXF. Blackmagic RAW abre com o Blackmagic RAW SDK, gratuito.
integration-extra-group-other-images = SVG e outras imagens
# machine-drafted, needs native review
integration-extra-group-other-images-help = Imagens e documentos geralmente deixados para outros programas: desenhos SVG, PDF, XPS, DjVu, EPS, WMF e EMF, Visio, OpenDocument, WordPerfect Graphics, livros EPUB e fontes, e animações como MNG, FLIC e cursores animados.
integration-extra-group-text = Texto, tabelas e legendas
integration-extra-group-text-help = Texto simples e logs, NFO, Markdown, tabelas CSV e TSV, RTF e arquivos de legenda: SRT, WebVTT, ASS, SSA, MicroDVD, PGS, LRC, TTML e SCC.
