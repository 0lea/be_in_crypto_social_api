# 1. Creazione Like (User 1)
POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_1}}
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 201
[Asserts]
jsonpath "$.liked" == true
jsonpath "$.count" >= 1

# 2. Test Idempotenza (Stesso Like)
POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_1}}
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 201
[Asserts]
jsonpath "$.already_existed" == true
jsonpath "$.count" >= 1

# 3. Verifica Stato e Count
GET {{base_url}}/v1/likes/post/{{post_id}}/status
Authorization: Bearer {{tok_user_1}}
HTTP 200
[Asserts]
jsonpath "$.liked" == true

GET {{base_url}}/v1/likes/post/{{post_id}}/count
HTTP 200
[Asserts]
jsonpath "$.count" >= 1

# 4. Unlike (Rimozione)
DELETE {{base_url}}/v1/likes/post/{{post_id}}
Authorization: Bearer {{tok_user_1}}
HTTP 200
[Asserts]
jsonpath "$.was_liked" == true
