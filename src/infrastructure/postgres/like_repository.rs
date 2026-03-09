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
    async fn save(&self, like: &Like) -> Result<(), DomainError> {
        let content_id = like.content_id.0;
        let user_id = like.user_id.0;
        let content_type = like.content_type.as_str();

        sqlx::query!(
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
        .map_err(|e| {
            tracing::error!("Errore DB durante il salvataggio like: {:?}", e);
            DomainError::DatabaseError(format!("Error during like {:?}", e))
        })?;

        Ok(())
    }

    async fn remove(
        &self,
        user_id: &UserId,
        content_type: &ContentType,
        content_id: &ContentId,
    ) -> Result<(), DomainError> {
        sqlx::query!(
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
        .map_err(|_e| DomainError::DatabaseError("dsd".to_owned()))?;

        Ok(())
    }

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
