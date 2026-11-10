# machine-drafted, needs native review
# Specular — Library albums, smart albums and the date timeline.

## Sidebar
albums-title = Álbuns
albums-new = Novo álbum…
albums-new-smart = Novo álbum inteligente…
albums-save-search = Salvar a busca como álbum inteligente…
albums-smart = Álbum inteligente
# The album the photos go to when added without choosing.
albums-target = O que você adiciona vem para cá
albums-group-count = { $count ->
    [one] { $count } álbum
   *[other] { $count } álbuns
}
albums-need-library = Os álbuns ficam na Biblioteca. Adicione esta pasta à Biblioteca primeiro.

## Screen readers: the buttons of album and group rows
albums-a11y-rename = Renomear { $album }
albums-a11y-edit = Editar { $album }
albums-a11y-delete = Excluir o álbum { $album }
albums-a11y-rename-group = Renomear o grupo { $group }
albums-a11y-delete-group = Excluir o grupo { $group }

## Sheets
albums-name = Nome
albums-unnamed = Ainda sem nome
albums-new-title = Novo álbum
albums-new-smart-title = Novo álbum inteligente
albums-edit-smart-title = Editar álbum inteligente
albums-rename-title = Renomear álbum
albums-rename-group-title = Renomear grupo
albums-create = Criar
albums-rename = Renomear
albums-save = Salvar
albums-groups-note = Use / para colocá-lo em um grupo: Cliente X / seleção.
albums-name-empty = Digite um nome.
albums-name-taken = { $album } já existe.
albums-name-clash = { $album } é um álbum, então não pode ser também um grupo.

## Smart album rules
albums-rules = Regras
albums-rules-note = As regras valem de cima para baixo. Regras vazias são ignoradas.
albums-rules-none = Adicione pelo menos uma regra.
albums-rule-label = Regra { $number }
albums-rule-empty = Regra vazia
albums-rule-add = Adicionar uma regra
albums-rule-combine = Combinar com as regras acima
albums-rule-first = Fotos que atendem a
albums-rule-and = E
albums-rule-or = Ou
albums-rule-except = Exceto

## Adding and removing photos
albums-add-menu = Adicionar ao álbum…
albums-remove-menu = Remover do álbum
albums-add-title = { $count ->
    [one] Adicionar { $count } foto a um álbum
   *[other] Adicionar { $count } fotos a um álbum
}
albums-add = Adicionar
albums-add-new = Novo álbum
albums-add-list = Álbuns
albums-add-none = Ainda não há álbuns. Digite um nome para criar um.
albums-add-note = Ou digite um nome para um novo álbum.
albums-added = { $count ->
    [one] { $count } foto adicionada a { $album }.
   *[other] { $count } fotos adicionadas a { $album }.
}
albums-added-missing = { $count ->
    [one] { $count } não está na Biblioteca, então não foi adicionada.
   *[other] { $count } não estão na Biblioteca, então não foram adicionadas.
}
albums-created = { $album } criado.
albums-removed = { $count ->
    [one] { $count } foto removida de { $album }.
   *[other] { $count } fotos removidas de { $album }.
}
albums-remove = Remover
albums-remove-confirm = { $count ->
    [one] Remover { $count } foto de { $album }?
   *[other] Remover { $count } fotos de { $album }?
}
albums-remove-confirm-detail = Os arquivos continuam no disco e na Biblioteca.
albums-delete = Excluir
albums-delete-confirm = Excluir o álbum { $album }?
albums-delete-group-confirm = { $count ->
    [one] Excluir o grupo { $group } e seu { $count } álbum?
   *[other] Excluir o grupo { $group } e seus { $count } álbuns?
}
albums-delete-confirm-detail = As fotos continuam no disco e na Biblioteca.
albums-deleted = { $count ->
    [one] Álbum excluído.
   *[other] { $count } álbuns excluídos.
}
albums-failed = Não foi possível alterar os álbuns: { $reason }

## Date timeline
albums-timeline = Linha do tempo
albums-timeline-description = Escolha um período para mostrar só as fotos dele. Shift+clique escolhe vários seguidos.
albums-timeline-picked = Mostrando { $period }
albums-timeline-cleared = Mostrando todas as datas
