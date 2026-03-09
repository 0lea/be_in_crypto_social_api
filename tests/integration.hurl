

POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_1}}
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 201

# GET http://localhost:8000/v1/likes/post/731b0395-4888-4822-b516-05b4b7bf2089/count
# HTTP 200


# GET {{base_url}}/health/ready
# HTTP 200
# [Asserts]
# json "status" == "ok"

POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_1}}
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 201

# 3. Test Like User 2 (Idempotenza/Multipli)
POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_2}}
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 201

#  Verifica conteggio (deve essere almeno 2)
# GET {{base_url}}/v1/likes/post/{{post_id}}/count
# HTTP 200
# [Asserts]
# json "count" >= 2

#  Test Fallimento (Token inesistente nei mock)
POST {{base_url}}/v1/likes
Authorization: Bearer 32323232
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 401
