use crate::domain::{
    errors::DomainError,
    like::{ContentId, ContentType, Like, LikeDbRepository},
    user::UserId,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;

pub struct PostgresLikeRepository {
    pool: Arc<PgPool>,
}

impl PostgresLikeRepository {
    pub fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LikeDbRepository for PostgresLikeRepository {
    async fn save(&self, like: &Like) -> Result<bool, DomainError> {
        let content_id = like.content_id.0;
        let user_id = like.user_id.0;
        let content_type = like.content_type.as_str();

        let res = sqlx::query!(
            r#"
            INSERT INTO likes (user_id, content_type, content_id, created_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (user_id, content_type, content_id) DO NOTHING
            "#,
            user_id,
            content_type,
            content_id,
            like.created_at
        )
        .execute(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(format!("Error on like query {:?}", e)))?;

        Ok(res.rows_affected() == 0)
    }

    async fn get_like(
        &self,
        user_id: &UserId,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<Like, DomainError> {
        sqlx::query_as!(
            Like,
            r#"
            SELECT user_id, content_type, content_id, created_at FROM likes 
            WHERE user_id = $1 AND content_type = $2 AND content_id = $3
            "#,
            user_id.0,
            content_type.as_str(),
            content_id.0
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(e.to_string()))
    }
    async fn remove(
        &self,
        user_id: &UserId,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<bool, DomainError> {
        let result = sqlx::query!(
            r#"
            DELETE FROM likes 
            WHERE user_id = $1 AND content_type = $2 AND content_id = $3
            "#,
            user_id.0,
            content_type.as_str(),
            content_id.0
        )
        .execute(&*self.pool)
        .await
        .map_err(|e| DomainError::DatabaseError(format!("Error on unlike query {:?}", e)))?;

        Ok(result.rows_affected() > 0)
    }

    async fn get_likes_count(
        &self,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<u64, DomainError> {
        let count = sqlx::query_scalar!(
            r#"
            SELECT COUNT(*) 
            FROM likes 
            WHERE content_type = $1 AND content_id = $2
            "#,
            content_type.as_str(),
            content_id.0
        )
        .fetch_one(&*self.pool)
        .await
        .map_err(|e| {
            DomainError::DatabaseError(format!("Error on get like count query {:?}", e))
        })?;

        // if count.unwrap_or_default() == 0 {
        //     return Err(DomainError::DatabaseNotFound(
        //         "Error on get like count query, like not fond".to_string(),
        //     ));
        // }

        Ok(count.unwrap_or_default() as u64)
    }
    //TODO: use materialized table
    // async fn get_likes_count(&self, content_id: &ContentId) -> Result<u64, DomainError> {
    //     let count: i64 = sqlx::query_scalar!(
    //         "SELECT likes_count FROM content_stats WHERE content_id = $1",
    //         content_id.0
    //     )
    //     .fetch_one(&*self.pool)
    //     .await
    //     .map_err(|e| DomainError::DatabaseError(e.to_string()))?;
    //     Ok(count as u64)
    // }
    //
    //
    async fn get_user_likes(
        &self,
        user_id: &UserId,
        cursor: Option<DateTime<Utc>>,
        limit: u64,
    ) -> Result<Vec<Like>, DomainError> {
        let rows = sqlx::query!(
            r#"
        SELECT user_id, content_type, content_id, created_at
        FROM likes
        WHERE user_id = $1 
          AND ($2::timestamptz IS NULL OR created_at < $2)
        ORDER BY created_at DESC
        LIMIT $3
        "#,
            user_id.0,
            cursor,
            limit as i64
        )
        .fetch_all(&*self.pool)
        .await
        .map_err(|e| {
            tracing::error!("Errore paginazione: {:?}", e);
            DomainError::DatabaseError("dsd".to_owned())
        })?;

        Ok(rows
            .into_iter()
            .map(|row| Like {
                user_id: UserId(row.user_id),
                content_type: ContentType::new(&row.content_type),
                content_id: ContentId(row.content_id),
                created_at: row.created_at,
            })
            .collect())
    }
}
