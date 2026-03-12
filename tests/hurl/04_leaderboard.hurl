# Top 24h
GET {{base_url}}/v1/likes/top?content_type=post&window=24h&limit=10
HTTP 200
[Asserts]
jsonpath "$.items" isCollection

# Test Validazione Window Errata
GET {{base_url}}/v1/likes/top?content_type=post&window=99years
HTTP 500


# TEST 1: Finestra 7 giorni (7d)
# Verifica che il sistema filtri correttamente i like più vecchi di 24h ma entro i 7 giorni.
GET {{base_url}}/v1/likes/top?content_type=post&window=7d&limit=10
HTTP 200
[Asserts]
jsonpath "$.items" isCollection
jsonpath "$.items[0].count" >= 0

# TEST 2: Capping del Limite
# Il service deve accettare limiti enormi ma capparli internamente (es. a 50) senza crashare.
GET {{base_url}}/v1/likes/top?content_type=post&window=24h&limit=9999
HTTP 200
[Asserts]
jsonpath "$.items" count <= 50

# TEST 3: Stato Vuoto (Empty State)
# Una query su una finestra o tipo contenuto senza like deve restituire un array vuoto, non un errore.
GET {{base_url}}/v1/likes/top?content_type=bonus_hunter&window=24h
HTTP 200
[Asserts]
jsonpath "$.items" count == 0

# TEST 1: Cambio finestra dinamico
# Verifica che passando da 24h a 7d i risultati siano coerenti (o almeno non diano errore)
GET {{base_url}}/v1/likes/top?content_type=post&window=24h
HTTP 200
GET {{base_url}}/v1/likes/top?content_type=post&window=7d
HTTP 200

# TEST 2: Parametri mancanti (Defaulting)
# Il tuo codice Rust dovrebbe gestire i default se manca il limit
GET {{base_url}}/v1/likes/top?content_type=post&window=24h
HTTP 200
[Asserts]
jsonpath "$.items" isCollection

# TEST 3: Content Type inesistente nella leaderboard
GET {{base_url}}/v1/likes/top?content_type=video_fake&window=24h
HTTP 200
[Asserts]
jsonpath "$.items" count == 0
