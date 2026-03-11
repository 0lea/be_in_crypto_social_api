use axum::http::StatusCode;
use axum_test::TestServer;
use redis::Commands;
use serde_json::{Value, json};
use sqlx::PgPool;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::task::JoinSet;
use uuid::Uuid;
use wiremock::{
    Mock, MockServer, ResponseTemplate,
    matchers::{header, method, path},
};

use social_api::{
    application::like_service::LikeService,
    create_app,
    domain::{
        like::{ContentId, ContentType},
        user::UserId,
    },
    infrastructure::{
        clients::http_external_validator::HttpExternalValidator,
        postgres::like_repository::PostgresLikeRepository,
        redis::like_repository::RedisLikeRepository,
    },
};

async fn auth_ok(user_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/v1/auth/validate"))
        .and(header("Authorization", format!("Bearer {}", user_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "valid": true,
        "user_id": user_id,
        "display_name": "test_user"
        })))
        .mount(&mock_server)
        .await;
}

async fn auth_err(user_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/v1/auth/validate"))
        .and(header("Authorization", format!("Bearer {}", user_id)))
        .respond_with(ResponseTemplate::new(401))
        .mount(&mock_server)
        .await;
}

async fn post_err(post_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/post/{}", post_id)))
        .respond_with(ResponseTemplate::new(404))
        .mount(&mock_server)
        .await;
}

async fn bonus_hunter_ok(post_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/bonus_hunter/{}", post_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id": post_id,
        "content_type": "bonus_hunter",
            })))
        .mount(&mock_server)
        .await;
}

async fn post_ok(post_id: &str, mock_server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/post/{}", post_id)))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
        "id": post_id,
        "content_type": "post",
            })))
        .mount(&mock_server)
        .await;
}

async fn setup_test_app(pool: PgPool, mock_url: String) -> axum::Router {
    dotenvy::dotenv().ok();
    let redis_url = std::env::var("REDIS_TEST_URL").unwrap_or("redis://127.0.0.1:6379/2".into());
    let redis_client = redis::Client::open(redis_url).unwrap();

    let mut conn = redis_client
        .get_connection()
        .expect("Failed to connect to Redis for cleanup");
    let _: () = redis::cmd("FLUSHDB")
        .query(&mut conn)
        .expect("Failed to flush Redis");

    let db_repo = Arc::new(PostgresLikeRepository::new(Arc::new(pool)));
    let cache_repo = Arc::new(RedisLikeRepository::new(Arc::new(redis_client)));
    let mut content_apis = HashMap::new();
    content_apis.insert("post".to_string(), mock_url.clone());
    content_apis.insert("bonus_hunter".to_string(), mock_url.clone());
    let validator = Arc::new(HttpExternalValidator::new(mock_url.clone(), content_apis));
    let like_service = Arc::new(LikeService::new(db_repo, cache_repo, validator.clone()));
    create_app(like_service, validator)
}

#[sqlx::test]
#[test_log::test]
async fn test_full_like_lifecycle(pool: PgPool) {
    let mock_server = MockServer::start().await;
    let app = setup_test_app(pool, mock_server.uri()).await;
    let server = TestServer::new(app);

    let test_user_id = "550e8400-e29b-41d4-a716-446655440001";
    let test_post_id = &uuid::Uuid::new_v4().to_string();

    auth_ok(test_user_id, &mock_server).await;
    post_ok(test_post_id, &mock_server).await;

    // 1. create Like
    let res = server
        .post("/v1/likes")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .json(&json!({"content_type": "post", "content_id": test_post_id}))
        .await;
    res.assert_status(StatusCode::CREATED);
    assert_eq!(res.json::<Value>()["liked"], true);
    assert_eq!(res.json::<Value>()["count"], 1);

    // indeponent
    let res = server
        .post("/v1/likes")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .json(&json!({"content_type": "post", "content_id": test_post_id}))
        .await;
    res.assert_status(StatusCode::CREATED);
    assert_eq!(res.json::<Value>()["already_existed"], true);
    assert_eq!(res.json::<Value>()["count"], 1);

    // Stato
    let res = server
        .get(&format!("/v1/likes/post/{}/status", test_post_id))
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .await;
    res.assert_status(StatusCode::OK);
    assert_eq!(res.json::<Value>()["liked"], true);

    // recount
    let res = server
        .get(&format!("/v1/likes/post/{}/count", test_post_id))
        .await;
    res.assert_status(StatusCode::OK);
    assert_eq!(res.json::<Value>()["content_type"], "post");
    assert_eq!(res.json::<Value>()["content_id"], test_post_id.as_str());
    assert_eq!(res.json::<Value>()["count"], 1);

    // 4. Rimozione Like (Unlike)
    let res = server
        .delete(&format!("/v1/likes/post/{}", test_post_id))
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .await;
    res.assert_status(StatusCode::OK);
    assert_eq!(res.json::<Value>()["was_liked"], true);
    assert_eq!(res.json::<Value>()["count"], 0);
}

#[sqlx::test]
#[test_log::test]
async fn test_edge_cases(pool: PgPool) {
    let mock_server = MockServer::start().await;
    let app = setup_test_app(pool, mock_server.uri()).await;
    let server = TestServer::new(app);

    let test_user_id = "550e8400-e29b-41d4-a716-446655440001";
    let test_user_id_2 = "550e8400-e29b-41d4-a716-446655440002";
    let test_post_id = &uuid::Uuid::new_v4().to_string();

    // unexistent
    auth_ok(test_user_id, &mock_server).await;
    post_err(test_post_id, &mock_server).await;

    let res = server
        .post("/v1/likes")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .json(&json!({"content_type": "post", "content_id": test_post_id}))
        .await;
    res.assert_status(StatusCode::NOT_FOUND);

    // Unlike of unexistent like
    let res = server
        .delete(&format!("/v1/likes/post/{}", test_post_id))
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .await;
    res.assert_status(StatusCode::NOT_FOUND);

    // create one
    mock_server.reset().await;
    auth_ok(test_user_id, &mock_server).await;
    post_ok(test_post_id, &mock_server).await;
    let res = server
        .post("/v1/likes")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .json(&json!({"content_type": "post", "content_id": test_post_id}))
        .await;
    res.assert_status(StatusCode::CREATED);
    assert_eq!(res.json::<Value>()["liked"], true);
    assert_eq!(res.json::<Value>()["count"], 1);

    // user 2 delete unliked post
    auth_ok(test_user_id_2, &mock_server).await;
    let res = server
        .delete(&format!("/v1/likes/post/{}", test_post_id))
        .add_header("Authorization", format!("Bearer {}", test_user_id_2))
        .await;
    res.assert_status(StatusCode::OK);
    assert_eq!(res.json::<Value>()["was_liked"], false);

    // Token Invalid
    auth_err("tok_bad", &mock_server).await;
    let res = server
        .post("/v1/likes")
        .add_header("Authorization", "Bearer tok_bad")
        .json(&json!({"content_type": "post", "content_id": test_post_id}))
        .await;
    res.assert_status(StatusCode::UNAUTHORIZED);
}

#[sqlx::test]
#[test_log::test]
async fn test_batch_count(pool: PgPool) {
    let mock_server = MockServer::start().await;
    let app = setup_test_app(pool, mock_server.uri()).await;
    let server = TestServer::new(app);

    let test_post_id = &uuid::Uuid::new_v4().to_string();
    let test_user_id = "550e8400-e29b-41d4-a716-446655440001";
    let test_user_id_2 = "550e8400-e29b-41d4-a716-446655440002";

    auth_ok(test_user_id, &mock_server).await;
    auth_ok(test_user_id_2, &mock_server).await;
    post_ok(test_post_id, &mock_server).await;

    let mut expected_res = HashMap::with_capacity(100);

    for j in 0..100 {
        let is_even = j % 2 == 0;

        let post_id = uuid::Uuid::new_v4().to_string();

        let mut count = 0;

        post_ok(&post_id, &mock_server).await;

        // create 1
        let res = server
            .post("/v1/likes")
            .add_header("Authorization", format!("Bearer {}", test_user_id))
            .json(&json!({"content_type": "post", "content_id": post_id}))
            .await;
        res.assert_status(StatusCode::CREATED);
        assert_eq!(res.json::<Value>()["liked"], true);
        assert_eq!(res.json::<Value>()["count"], 1);

        count += 1;

        if is_even {
            let res = server
                .post("/v1/likes")
                .add_header("Authorization", format!("Bearer {}", test_user_id_2))
                .json(&json!({"content_type": "post", "content_id": post_id}))
                .await;
            res.assert_status(StatusCode::CREATED);
            assert_eq!(res.json::<Value>()["liked"], true);
            assert_eq!(res.json::<Value>()["count"], 2);
            count += 1;
        }
        expected_res.insert(post_id, count);
    }

    let batch_items: Vec<Value> = expected_res
        .keys()
        .map(|id| {
            json!({
                "content_type": "post",
                "content_id": id
            })
        })
        .collect();

    let res = server
        .post("/v1/likes/batch/counts")
        .json(&json!({ "items": batch_items }))
        .await;
    res.assert_status(StatusCode::OK);

    let actual_res: Vec<Value> = res.json();
    assert_eq!(actual_res.len(), 100, "must have 100 elements");

    for item in actual_res {
        let content_id = item["content_id"].as_str().unwrap();
        let actual_count = item["count"].as_u64().unwrap();

        let expected_count = expected_res.get(content_id).expect("ID not found");

        assert_eq!(
            actual_count, *expected_count,
            "wrong count for post {}",
            content_id
        );
    }

    // flush redis
    let redis_url = std::env::var("REDIS_TEST_URL").unwrap_or("redis://127.0.0.1:6379/2".into());
    let redis_client = redis::Client::open(redis_url).unwrap();
    let mut conn = redis_client
        .get_connection()
        .expect("Failed to connect to Redis for cleanup");
    let _: () = redis::cmd("FLUSHDB")
        .query(&mut conn)
        .expect("Failed to flush Redis");

    // retest with DB fallback
    let res = server
        .post("/v1/likes/batch/counts")
        .json(&json!({ "items": batch_items }))
        .await;
    res.assert_status(StatusCode::OK);

    let actual_res: Vec<Value> = res.json();
    assert_eq!(actual_res.len(), 100, "must have 100 elements");

    // let the fire and forget refresh work
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    for item in actual_res {
        let content_id = item["content_id"].as_str().unwrap();
        let actual_count = item["count"].as_u64().unwrap();

        let expected_count = expected_res.get(content_id).expect("ID not found");

        assert_eq!(
            actual_count, *expected_count,
            "wrong count for post {}",
            content_id
        );
    }
}

#[sqlx::test]
#[test_log::test]
async fn test_batch_statuses(pool: PgPool) {
    let mock_server = MockServer::start().await;
    let app = setup_test_app(pool, mock_server.uri()).await;
    let server = TestServer::new(app);

    let test_user_id = "550e8400-e29b-41d4-a716-446655440001";

    let mut liked_ids = std::collections::HashSet::new();
    let mut all_items = Vec::new();

    auth_ok(test_user_id, &mock_server).await;

    for i in 0..100 {
        let content_id = uuid::Uuid::new_v4().to_string();
        let content_type = if i % 2 == 0 { "post" } else { "bonus_hunter" };

        match content_type {
            "post" => post_ok(&content_id, &mock_server).await,
            "bonus_hunter" => {
                bonus_hunter_ok(&content_id, &mock_server).await;
            }
            _ => post_ok(&content_id, &mock_server).await,
        }

        all_items.push(json!({
            "content_type": content_type,
            "content_id": content_id
        }));

        if i < 50 {
            let res = server
                .post("/v1/likes")
                .add_header("Authorization", format!("Bearer {}", test_user_id))
                .json(&json!({
                    "content_type": content_type,
                    "content_id": content_id
                }))
                .await;

            res.assert_status(StatusCode::CREATED);
            liked_ids.insert(content_id);
        }
    }

    let res = server
        .post("/v1/likes/batch/statuses")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .json(&json!({ "items": all_items }))
        .await;

    res.assert_status(StatusCode::OK);

    let body: Value = res.json();
    let results = body["results"]
        .as_array()
        .expect("results must be an array");

    assert_eq!(
        results.len(),
        100,
        "La risposta deve contenere esattamente 100 elementi"
    );

    for item in results {
        let c_id = item["content_id"].as_str().expect("ID mancante");
        let is_liked = item["liked"].as_bool().expect("liked field mancante");
        let liked_at = &item["liked_at"];

        if liked_ids.contains(c_id) {
            assert!(
                is_liked,
                "Il contenuto {} doveva risultare liked: true",
                c_id
            );
            assert!(
                !liked_at.is_null(),
                "Il contenuto {} doveva avere un timestamp liked_at",
                c_id
            );
        } else {
            assert!(
                !is_liked,
                "Il contenuto {} doveva risultare liked: false",
                c_id
            );
            assert!(
                liked_at.is_null(),
                "Il contenuto {} NON doveva avere un timestamp",
                c_id
            );
        }
    }

    let other_user = "550e8400-e29b-41d4-a716-446655440003";
    auth_ok(other_user, &mock_server).await;

    let res_other = server
        .post("/v1/likes/batch/statuses")
        .add_header("Authorization", format!("Bearer {}", other_user))
        .json(&json!({ "items": all_items }))
        .await;

    let body_other: Value = res_other.json();
    for item in body_other["results"].as_array().unwrap() {
        assert_eq!(
            item["liked"], false,
            "L'utente 2 non dovrebbe vedere i like dell'utente 1"
        );
    }
}

#[sqlx::test]
#[test_log::test]
async fn test_user_likes_pagination_and_filtering(pool: PgPool) {
    let mock_server = MockServer::start().await;
    let app = setup_test_app(pool, mock_server.uri()).await;
    let server = TestServer::new(app);

    let test_user_id = uuid::Uuid::new_v4().to_string();
    auth_ok(&test_user_id, &mock_server).await;

    // 1. Setup: Creiamo 25 likes (15 post, 10 bonus_hunter)
    // Li inseriamo con un piccolo delay o in ordine per testare il sorting DESC
    for i in 0..25 {
        let content_id = uuid::Uuid::new_v4().to_string();
        let content_type = if i < 15 { "post" } else { "bonus_hunter" };

        // Mock dei servizi esterni
        if i < 15 {
            post_ok(&content_id, &mock_server).await;
        } else {
            bonus_hunter_ok(&content_id, &mock_server).await;
        }

        server
            .post("/v1/likes")
            .add_header("Authorization", format!("Bearer {}", test_user_id))
            .json(&json!({ "content_type": content_type, "content_id": content_id }))
            .await
            .assert_status(StatusCode::CREATED);

        // Un piccolo sleep non guasta per garantire timestamp diversi se il DB è troppo veloce,
        // anche se il nostro cursore gestisce i duplicati tramite ID.
        tokio::time::sleep(tokio::time::Duration::from_millis(2)).await;
    }

    // --- CASE A: Paginazione completa (senza filtri) ---
    // Pagina 1: primi 10
    let res1 = server
        .get("/v1/likes/user")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .add_query_params(json!({ "limit": 10 }))
        .await;

    res1.assert_status(StatusCode::OK);
    let body1: Value = res1.json();
    let items1 = body1["items"].as_array().unwrap();
    let next_cursor = body1["next_cursor"]
        .as_str()
        .expect("Manca il cursore per la pag 2");

    assert_eq!(items1.len(), 10);

    // Pagina 2: altri 10 usando il cursore
    let res2 = server
        .get("/v1/likes/user")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .add_query_params(json!({ "limit": 10, "cursor": next_cursor }))
        .await;

    let body2: Value = res2.json();
    let items2 = body2["items"].as_array().unwrap();
    let next_cursor_2 = body2["next_cursor"]
        .as_str()
        .expect("Manca il cursore per la pag 3");

    assert_eq!(items2.len(), 10);
    // Verifichiamo che non ci siano duplicati tra le pagine
    assert_ne!(items1[0]["content_id"], items2[0]["content_id"]);

    // Pagina 3: ultimi 5
    let res3 = server
        .get("/v1/likes/user")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .add_query_params(json!({ "limit": 10, "cursor": next_cursor_2 }))
        .await;

    let body3: Value = res3.json();
    assert_eq!(body3["items"].as_array().unwrap().len(), 5);
    assert!(
        body3["next_cursor"].is_null(),
        "L'ultima pagina non deve avere un cursore"
    );

    // --- CASE B: Filtraggio per content_type ---
    let res_filter = server
        .get("/v1/likes/user")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .add_query_params(json!({ "content_type": "bonus_hunter", "limit": 50 }))
        .await;

    let items_filtered = res_filter.json::<Value>()["items"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(
        items_filtered, 10,
        "Dovrebbero esserci solo 10 bonus_hunter"
    );

    // --- CASE C: Validazione Parametri (Strict) ---
    // 1. Limit mancante (se lo abbiamo reso obbligatorio, deve dare 400)
    let res_no_limit = server
        .get("/v1/likes/user")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .await;
    res_no_limit.assert_status(StatusCode::OK);

    // 2. Cursore malformato (Base64 invalido o JSON rotto)
    let res_bad_cursor = server
        .get("/v1/likes/user")
        .add_header("Authorization", format!("Bearer {}", test_user_id))
        .add_query_params(json!({ "limit": 10, "cursor": "not-base64-content" }))
        .await;
    // Qui dipende da come gestisci l'errore nel service, tipicamente 400
    res_bad_cursor.assert_status(StatusCode::BAD_REQUEST);

    // --- CASE D: Isolamento Utenti ---
    let other_user = uuid::Uuid::new_v4().to_string();
    auth_ok(&other_user, &mock_server).await;

    let res_empty = server
        .get("/v1/likes/user")
        .add_header("Authorization", format!("Bearer {}", other_user))
        .add_query_params(json!({ "limit": 10 }))
        .await;

    assert_eq!(
        res_empty.json::<Value>()["items"].as_array().unwrap().len(),
        0
    );
}

async fn insert_old_like(
    pool: &PgPool,
    user_id: UserId,
    content_id: ContentId,
    c_type: ContentType,
    hours_ago: i32,
) {
    sqlx::query!(
        "INSERT INTO likes (user_id, content_id, content_type, created_at) 
         VALUES ($1, $2, $3, NOW() - make_interval(hours => $4))",
        user_id.0,
        content_id.0,
        c_type.as_str(),
        hours_ago
    )
    .execute(pool)
    .await
    .unwrap();
}

#[sqlx::test]
#[test_log::test]
async fn test_leaderboard_full_lifecycle(pool: PgPool) {
    let mock_server = MockServer::start().await;
    let app = setup_test_app(pool.clone(), mock_server.uri()).await;
    let server = TestServer::new(app);

    let redis_url = std::env::var("REDIS_TEST_URL").unwrap_or("redis://127.0.0.1:6379/2".into());
    let redis_client = redis::Client::open(redis_url).unwrap();
    let mut conn = redis_client
        .get_connection()
        .expect("Failed to connect to Redis");

    let post_recent_id = Uuid::new_v4();
    let post_old_id = Uuid::new_v4();
    let new_user_id = || -> UserId { Uuid::new_v4().into() };

    for _ in 0..10 {
        insert_old_like(
            &pool,
            new_user_id(),
            post_recent_id.into(),
            "post".to_string().into(),
            1,
        )
        .await;
    }
    for _ in 0..15 {
        insert_old_like(
            &pool,
            new_user_id(),
            post_old_id.into(),
            "post".to_string().into(),
            48,
        )
        .await;
    }

    // --- CASE 1: Cold Start (Redis Vuoto) ---
    let _: () = redis::cmd("FLUSHDB").query(&mut conn).unwrap();

    let res = server
        .get("/v1/likes/top")
        .add_query_params(json!({ "content_type": "post", "window": "24h", "limit": 10 }))
        .await;

    res.assert_status(StatusCode::OK);
    let body: Value = res.json();
    assert_eq!(body["items"][0]["content_id"], post_recent_id.to_string());
    assert_eq!(body["items"][0]["count"], 10);

    // --- CASE 2: Cache Hit ---
    // Aggiungiamo un like (totale 11), ma la cache deve restituire ancora 10
    insert_old_like(
        &pool,
        new_user_id(),
        post_recent_id.into(),
        "post".to_string().into(),
        0,
    )
    .await;

    let res_cached = server
        .get("/v1/likes/top")
        .add_query_params(json!({ "content_type": "post", "window": "24h" }))
        .await;
    assert_eq!(res_cached.json::<Value>()["items"][0]["count"], 10);

    // --- CASE 3: Canary Expired & Background Refresh ---
    // Il refresh avviene se il canary non c'è. La chiave dipende dalla tua implementazione del repo.
    let _: () = conn.del("leaderboard:canary:24h:post").unwrap();

    // Serve ancora dati vecchi (10)
    let res_stale = server
        .get("/v1/likes/top")
        .add_query_params(json!({ "content_type": "post", "window": "24h" }))
        .await;
    assert_eq!(res_stale.json::<Value>()["items"][0]["count"], 10);

    // Aspettiamo il task background
    tokio::time::sleep(Duration::from_millis(150)).await;

    // Ora deve essere aggiornato a 11
    let res_fresh = server
        .get("/v1/likes/top")
        .add_query_params(json!({ "content_type": "post", "window": "24h" }))
        .await;
    assert_eq!(res_fresh.json::<Value>()["items"][0]["count"], 11);

    // --- CASE 4: Thundering Herd & Lock ---
    let _: () = conn.del("leaderboard:canary:24h:post").unwrap();

    let mut set = JoinSet::new();
    let srv_arc = Arc::new(server);
    for _ in 0..10 {
        let s = srv_arc.clone();
        set.spawn(async move {
            s.get("/v1/likes/top")
                .add_query_params(json!({ "content_type": "post", "window": "24h" }))
                .await
        });
    }

    while let Some(res) = set.join_next().await {
        res.unwrap().assert_status(StatusCode::OK);
    }

    // --- CASE 5: Finestre temporali diverse (7d) ---
    let res_7d = srv_arc
        .get("/v1/likes/top")
        .add_query_params(json!({ "content_type": "post", "window": "7d" }))
        .await;
    let body_7d: Value = res_7d.json();
    // In 7 giorni vince il post vecchio con 15 like
    assert_eq!(body_7d["items"][0]["content_id"], post_old_id.to_string());
    assert_eq!(body_7d["items"][0]["count"], 15);
}

#[sqlx::test]
async fn test_leaderboard_validation(pool: PgPool) {
    let mock_server = MockServer::start().await;
    let app = setup_test_app(pool, mock_server.uri()).await;
    let server = TestServer::new(app);

    // Window invalida tramite query param
    server
        .get("/v1/likes/top")
        .add_query_param("window", "invalid_window")
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);

    // Limit enorme: il service lo cappa a 50, deve rispondere 200 OK
    server
        .get("/v1/likes/top")
        .add_query_params(json!({ "window": "24h", "limit": 9999 }))
        .await
        .assert_status(StatusCode::OK);
}
#[sqlx::test]
async fn test_leaderboard_empty_states(pool: PgPool) {
    let mock_server = MockServer::start().await;
    let app = setup_test_app(pool, mock_server.uri()).await;
    let server = TestServer::new(app);

    let res = server
        .get("/v1/likes/top")
        .add_query_params(json!({ "window": "24h", "limit": 9999 }))
        .await;
    res.assert_status(StatusCode::OK);
    assert_eq!(res.json::<Value>()["items"].as_array().unwrap().len(), 0);
}
