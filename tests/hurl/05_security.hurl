# Like su contenuto inesistente (404 ritornato dal validatore esterno)
POST {{base_url}}/v1/likes
Authorization: Bearer {{tok_user_1}}
{
  "content_type": "post",
  "content_id": "{{invalid_post_id}}"
}
HTTP 404

# Unlike di un contenuto mai cliccato (Deve rispondere 200 o 404 in base alla tua logica, 
# dai test Rust sembra che rispondi OK con was_liked: false)
DELETE {{base_url}}/v1/likes/post/{{invalid_post_id}}
Authorization: Bearer {{tok_user_1}}
HTTP 200
[Asserts]
jsonpath "$.was_liked" == false

# Token Malformato
POST {{base_url}}/v1/likes
Authorization: Bearer token_spazzatura
{
  "content_type": "post",
  "content_id": "{{post_id}}"
}
HTTP 401



# TEST 2: Formato ID Invalido
# Se invii un ID che non è un UUID valido, il validatore o il routing devono bloccarlo.
GET {{base_url}}/v1/likes/post/not-a-uuid/count
HTTP 404

# TEST 3: Token Invalido/Scaduto
# Qualsiasi tentativo di azione protetta con token errato deve fallire con 401.
POST {{base_url}}/v1/likes
Authorization: Bearer tok_bad
{ "content_type": "post", "content_id": "{{post_id}}" }
HTTP 401
