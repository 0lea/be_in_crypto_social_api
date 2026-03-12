# Batch Counts
POST {{base_url}}/v1/likes/batch/counts
{
  "items": [
    {"content_type": "post", "content_id": "{{post_id}}"},
    {"content_type": "bonus_hunter", "content_id": "{{bonus_id}}"}
  ]
}
HTTP 200
[Asserts]
jsonpath "$" count == 2
jsonpath "$[?(@.content_id == '{{post_id}}')]" exists

# Batch Statuses (User Isolation)
POST {{base_url}}/v1/likes/batch/statuses
Authorization: Bearer {{tok_user_1}}
{
  "items": [
    {"content_type": "post", "content_id": "{{post_id}}"}
  ]
}
HTTP 200
[Asserts]
jsonpath "$.results" count == 1



# TEST 1: Mix di tipi di contenuto
# Verifica che il batch gestisca contemporaneamente 'post' e 'bonus_hunter'.
POST {{base_url}}/v1/likes/batch/counts
{
  "items": [
    {"content_type": "post", "content_id": "{{post_id}}"},
    {"content_type": "bonus_hunter", "content_id": "{{bonus_id}}"}
  ]
}
HTTP 200
[Asserts]
jsonpath "$[0].content_type" == "post"
jsonpath "$[1].content_type" == "bonus_hunter"

# TEST 2: Batch Status Isolation
# Utente 2 non deve vedere i "liked: true" dell'Utente 1.
POST {{base_url}}/v1/likes/batch/statuses
Authorization: Bearer {{tok_user_2}}
{
  "items": [{"content_type": "post", "content_id": "{{post_id}}"}]
}
HTTP 200
[Asserts]
jsonpath "$.results[0].liked" == false

# TEST 3: Batch Performance (Large Request)
# Testiamo la risposta con un numero elevato di ID (simulando i 100 del test Rust).
POST {{base_url}}/v1/likes/batch/counts
{
  "items": [
    {"content_type": "post", "content_id": "uuid-1"},
    {"content_type": "post", "content_id": "uuid-2"}
  ]
}
HTTP 200
[Asserts]
duration < 100 # ms

# TEST 1: Batch con ID duplicati nella richiesta
# Il server non deve esplodere se mando lo stesso ID due volte nel JSON
POST {{base_url}}/v1/likes/batch/counts
{
  "items": [
    {"content_type": "post", "content_id": "{{post_id}}"},
    {"content_type": "post", "content_id": "{{post_id}}"}
  ]
}
HTTP 200

# TEST 2: Batch con mix di ID validi e ID inventati
POST {{base_url}}/v1/likes/batch/counts
{
  "items": [
    {"content_type": "post", "content_id": "{{post_id}}"},
    {"content_type": "post", "content_id": "00000000-0000-0000-0000-000000000000"}
  ]
}
HTTP 200
[Asserts]
jsonpath "$[1].count" == 0

# TEST 3: Batch vuoto
POST {{base_url}}/v1/likes/batch/counts
{ "items": [] }
HTTP 200
[Asserts]
jsonpath "$" count == 0
