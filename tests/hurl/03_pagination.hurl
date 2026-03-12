# Richiesta paginata: Pagina 1
GET {{base_url}}/v1/likes/user?limit=2&content_type=post
Authorization: Bearer {{tok_user_1}}
HTTP 200
[Asserts]
jsonpath "$.items" count <= 2
# Catturiamo il cursore per la prossima chiamata
[Captures]
cursor: jsonpath "$.next_cursor"

# Richiesta paginata: Pagina 2 (se il cursore esiste)
GET {{base_url}}/v1/likes/user?limit=2&cursor={{cursor}}
Authorization: Bearer {{tok_user_1}}
HTTP 200

# TEST 1: Filtraggio per Content Type
# Recupera solo i like dell'utente relativi ai 'bonus_hunter'.
GET {{base_url}}/v1/likes/user?content_type=bonus_hunter&limit=50
Authorization: Bearer {{tok_user_1}}
HTTP 200
[Asserts]
jsonpath "$.items[*].content_type" == "bonus_hunter"

# TEST 2: Validazione Cursore Corrotto
# Se il cursore non è un Base64 valido o è malformato, deve rispondere 400 Bad Request.
GET {{base_url}}/v1/likes/user?limit=10&cursor=not-base64-string
Authorization: Bearer {{tok_user_1}}
HTTP 400

# TEST 3: Isolamento Utenti (Empty List)
# Un utente nuovo deve ricevere una lista vuota e nessun cursore.
GET {{base_url}}/v1/likes/user?limit=10
Authorization: Bearer {{tok_user_3}}
HTTP 200
[Asserts]
jsonpath "$.items" count == 0
jsonpath "$.next_cursor" == null

# TEST 1: Limit a 1 (Stress del cursore)
# Forza il server a generare un cursore anche con un solo elemento
GET {{base_url}}/v1/likes/user?limit=1
Authorization: Bearer {{tok_user_1}}
HTTP 200
[Asserts]
jsonpath "$.next_cursor" != null

# TEST 2: Filtro combinato (Type + Limit)
GET {{base_url}}/v1/likes/user?content_type=post&limit=5
Authorization: Bearer {{tok_user_1}}
HTTP 200
[Asserts]
jsonpath "$.items[*].content_type" == "post"

# TEST 3: Cursore su lista vuota
# Se chiedo una pagina successiva dove non c'è più nulla
GET {{base_url}}/v1/likes/user?cursor=some_final_cursor_base64
Authorization: Bearer {{tok_user_1}}
HTTP 200
[Asserts]
jsonpath "$.items" count == 0
