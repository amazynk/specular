# O editor de metadados: campos IPTC das fotografias selecionadas (galeria, Biblioteca, modo de triagem e
# o visualizador).

palette-edit-metadata = Editar metadados
palette-edit-metadata-help = Editar o título, a legenda, o autor, os direitos e as localizações das fotografias selecionadas.
metaedit-menu = Editar metadados…
metaedit-title = Metadados
metaedit-close = Fechar metadados
metaedit-photos = { $count ->
    [one] 1 fotografia
   *[other] { $count } fotografias
}
metaedit-none-selected = Selecione fotografias para editar os seus metadados.
metaedit-reading = A ler metadados…
metaedit-mixed = Vários valores: escreva para definir em todas
metaedit-clearing = Apagado em todas as fotografias. Esc mantém os valores.
metaedit-keywords-others = algumas têm mais
metaedit-saved = Guardado: { $field }.
metaedit-cleared = Apagado: { $field }.
metaedit-write-failed = { $count ->
    [one] Metadados não guardados em 1 fotografia: { $error }
   *[other] Metadados não guardados em { $count } fotografias: { $error }
}
metaedit-writes-sidecars = Guardados em ficheiros XMP.
metaedit-writes-files = Guardados em ficheiros JPEG, TIFF e DNG; os outros ficheiros recebem ficheiros XMP.

metaedit-group-templates = Modelos
metaedit-group-description = Descrição
metaedit-group-creator = Autor
metaedit-group-rights = Direitos
metaedit-group-shown = Local mostrado
metaedit-group-created = Local de criação
metaedit-group-workflow = Fluxo de trabalho
metaedit-templates-none = Ainda não há modelos. Um modelo preenche os campos que usa com frequência.

metaedit-field-title = Título
metaedit-field-headline = Cabeçalho
metaedit-field-description = Legenda
metaedit-field-keywords = Palavras-chave (separadas por ;)
metaedit-field-persons-shown = Pessoas mostradas (separadas por ;)
metaedit-field-event = Evento
metaedit-field-creator = Autor (separados por ;)
metaedit-field-creator-job-title = Cargo do autor
metaedit-field-contact-address = Morada
metaedit-field-contact-city = Cidade
metaedit-field-contact-region = Distrito ou região
metaedit-field-contact-postal-code = Código postal
metaedit-field-contact-country = País
metaedit-field-contact-phone = Telefone
metaedit-field-contact-email = E-mail
metaedit-field-contact-url = Sítio Web
metaedit-field-copyright = Aviso de direitos de autor
metaedit-field-credit = Linha de crédito
metaedit-field-source = Fonte
metaedit-field-usage-terms = Condições de utilização
metaedit-field-shown-sublocation = Sublocal
metaedit-field-shown-city = Cidade
metaedit-field-shown-state = Distrito ou região
metaedit-field-shown-country = País
metaedit-field-shown-country-code = Código do país
metaedit-field-created-sublocation = Sublocal
metaedit-field-created-city = Cidade
metaedit-field-created-state = Distrito ou região
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
metaedit-template-hint = Os campos vazios não são alterados.
metaedit-template-replace-keywords = Substituir as palavras-chave das fotografias
metaedit-template-applied = { $count ->
    [one] «{ $name }» aplicado a 1 fotografia.
   *[other] «{ $name }» aplicado a { $count } fotografias.
}
metaedit-template-remove-ask = Remover o modelo «{ $name }»?
metaedit-template-remove-keep = As fotografias mantêm os seus metadados.
metaedit-template-remove-yes = Remover
metaedit-template-removed = Modelo «{ $name }» removido.

app-info-edit-metadata = Editar metadados…

settings-gallery-metadata-into-files = Escrever metadados nos ficheiros de imagem
settings-gallery-metadata-into-files-help = O editor de metadados escreve também nos ficheiros JPEG, TIFF e DNG, sem alterar mais nada neles. Desligado: só ficheiros XMP.
