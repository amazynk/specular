# The metadata editor: IPTC fields of the selected photos (gallery, Library, cull mode and
# the viewer).

palette-edit-metadata = Editar metadados
palette-edit-metadata-help = Edite o título, a legenda, o criador, os direitos e os locais das fotos selecionadas.
metaedit-menu = Editar metadados…
metaedit-title = Metadados
metaedit-close = Fechar metadados
metaedit-photos = { $count ->
    [one] { $count } foto
   *[other] { $count } fotos
}
metaedit-none-selected = Selecione fotos para editar os metadados delas.
metaedit-reading = Lendo metadados…
metaedit-mixed = Misto: digite para aplicar a todas
metaedit-clearing = Apagado em todas as fotos. Esc os mantém.
metaedit-keywords-others = algumas têm mais
metaedit-saved = { $field } salvo.
metaedit-cleared = { $field } apagado.
metaedit-write-failed = { $count ->
    [one] Metadados não salvos em { $count } foto: { $error }
   *[other] Metadados não salvos em { $count } fotos: { $error }
}
metaedit-writes-sidecars = Salvo em arquivos XMP auxiliares.
metaedit-writes-files = Salvo em arquivos JPEG, TIFF e DNG; os outros arquivos recebem arquivos XMP auxiliares.

metaedit-group-templates = Modelos
metaedit-group-description = Descrição
metaedit-group-creator = Criador
metaedit-group-rights = Direitos
metaedit-group-shown = Local mostrado
metaedit-group-created = Local de criação
metaedit-group-workflow = Fluxo de trabalho
metaedit-templates-none = Ainda não há modelos. Um modelo preenche os campos que você usa com frequência.

metaedit-field-title = Título
metaedit-field-headline = Manchete
metaedit-field-description = Legenda
metaedit-field-keywords = Palavras-chave (separadas por ;)
metaedit-field-persons-shown = Pessoas mostradas (separadas por ;)
metaedit-field-event = Evento
metaedit-field-creator = Criador (separados por ;)
metaedit-field-creator-job-title = Cargo do criador
metaedit-field-contact-address = Endereço
metaedit-field-contact-city = Cidade
metaedit-field-contact-region = Estado ou província
metaedit-field-contact-postal-code = CEP
metaedit-field-contact-country = País
metaedit-field-contact-phone = Telefone
metaedit-field-contact-email = E-mail
metaedit-field-contact-url = Site
metaedit-field-copyright = Aviso de copyright
metaedit-field-credit = Linha de crédito
metaedit-field-source = Fonte
metaedit-field-usage-terms = Termos de uso
metaedit-field-shown-sublocation = Sublocal
metaedit-field-shown-city = Cidade
metaedit-field-shown-state = Estado ou província
metaedit-field-shown-country = País
metaedit-field-shown-country-code = Código do país
metaedit-field-created-sublocation = Sublocal
metaedit-field-created-city = Cidade
metaedit-field-created-state = Estado ou província
metaedit-field-created-country = País
metaedit-field-created-country-code = Código do país
metaedit-field-instructions = Instruções
metaedit-field-job-id = ID do trabalho

metaedit-template-new = Novo modelo
metaedit-template-edit = Editar
metaedit-template-remove = Remover
metaedit-template-done = Concluído
metaedit-template-name = Nome do modelo
metaedit-template-default-name = Modelo { $n }
metaedit-template-title = Modelo
metaedit-template-hint = Campos vazios não são alterados.
metaedit-template-replace-keywords = Substituir as palavras-chave das fotos
metaedit-template-applied = { $count ->
    [one] “{ $name }” aplicado a { $count } foto.
   *[other] “{ $name }” aplicado a { $count } fotos.
}
metaedit-template-remove-ask = Remover o modelo “{ $name }”?
metaedit-template-remove-keep = As fotos mantêm seus metadados.
metaedit-template-remove-yes = Remover
metaedit-template-removed = Modelo “{ $name }” removido.

app-info-edit-metadata = Editar metadados…

settings-gallery-metadata-into-files = Gravar metadados nos arquivos de imagem
settings-gallery-metadata-into-files-help = O editor de metadados também grava em arquivos JPEG, TIFF e DNG, sem alterar mais nada neles. Desligado: só arquivos XMP auxiliares.
