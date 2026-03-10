#### POST LIKE

POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_1}}
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 201


POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_1}}
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 201


POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_2}}
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 201


POST {{base_url}}/v1/likes
Authorization: Bearer 32323232
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 401



#### DELETE LIKE

DELETE {{base_url}}/v1/likes/post/{{post_id}}
Authorization: Bearer {{tok_user_1}}
HTTP 200

# indepotent test
DELETE {{base_url}}/v1/likes/post/{{post_id}}
Authorization: Bearer {{tok_user_1}}
HTTP 200


DELETE {{base_url}}/v1/likes/post/{{post_id}}
Authorization: Bearer {{tok_user_2}}
HTTP 200


# Unauthorized
DELETE {{base_url}}/v1/likes/post/{{post_id}}
Authorization: Bearer 32323232
HTTP 401

# Not found
DELETE {{base_url}}/v1/likes/post/{{tok_user_1}}
Authorization: Bearer {{tok_user_1}}
HTTP 404



#### GET like count





GET {{base_url}}/v1/likes/post/{{post_id}}/count
HTTP 200
[Asserts]
header "Content-Type" contains "application/json"
jsonpath "$.content_type" == "post"
jsonpath "$.content_id" == "{{post_id}}"
jsonpath "$.count" isInteger
# Opzionale: verifica che arrivi dalla cache (se il tuo header lo indica)
# header "X-Cache" == "HIT"


# verify count, first 0 than 1
GET {{base_url}}/v1/likes/post/{{post_id}}/count
HTTP 200
[Asserts]
jsonpath "$.count" == 0

# create 1
POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_1}}
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 201

GET {{base_url}}/v1/likes/post/{{post_id}}/count
HTTP 200
[Asserts]
jsonpath "$.count" == 1

# Not exists
GET {{base_url}}/v1/likes/post/344b9bca-8333-4779-80ad-90ba00c13b69/count
HTTP 200
[Asserts]
jsonpath "$.count" == 0

# fake load test
GET {{base_url}}/v1/likes/post/{{post_id}}/count
HTTP 200
[Asserts]
duration < 5    # ms


### GET like status

GET {{base_url}}/v1/likes/post/{{post_id}}/status
Authorization: Bearer {{tok_user_1}}
HTTP 200
[Asserts]
jsonpath "$.liked" == true
jsonpath "$.liked_at" matches /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}.\d*Z$/


# 2. Verifica stato LIKE non presente (Utente 3 non ha messo like)
GET {{base_url}}/v1/likes/post/{{post_id}}/status
Authorization: Bearer {{tok_user_3}}
HTTP 200
[Asserts]
jsonpath "$.liked" == false
jsonpath "$.liked_at" == null


# 3. Tentativo senza Token (Errore di autenticazione)
GET {{base_url}}/v1/likes/post/{{post_id}}/status
HTTP 401


# 4. Tentativo con Token scaduto/invalido
GET {{base_url}}/v1/likes/post/{{post_id}}/status
Authorization: Bearer token_fasullo_123
HTTP 401


#  Verifica conteggio (deve essere almeno 2)
# GET {{base_url}}/v1/likes/post/{{post_id}}/count
# HTTP 200
# [Asserts]
# json "count" >= 2



# GET {{base_url}}/health/ready
# HTTP 200
# [Asserts]
# json "status" == "ok"

