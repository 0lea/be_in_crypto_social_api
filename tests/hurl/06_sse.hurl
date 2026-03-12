# TEST 1: Verifica Heartbeat
# Il server deve inviare un evento di "Heartbeat" per mantenere viva la connessione.
GET {{base_url}}/v1/likes/stream?content_type=post&content_id={{post_id}}
HTTP 200
[Asserts]
body contains "event: Heartbeat"

# TEST 2: Isolamento dei Canali (Negativo)
# Un like sul Post A non deve triggerare eventi per chi ascolta il Post B.
GET {{base_url}}/v1/likes/stream?content_type=post&content_id=differente-uuid
HTTP 200
[Asserts]
body not contains "content_id: {{post_id}}"

# TEST 3: Idempotenza Eventi
# Un secondo like dallo stesso utente (duplicato) non deve generare un nuovo evento SSE.
POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_1}}
{ "content_type": "post", "content_id": "{{post_id}}" }
HTTP 201
# (Eseguiamo un secondo POST)
POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_1}}
{ "content_type": "post", "content_id": "{{post_id}}" }
HTTP 201
# Nota: Qui verificheresti manualmente o con un tool di sniffing che l'evento SSE sia partito solo una volta.
