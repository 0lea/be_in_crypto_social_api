pub mod model;
pub mod repository;

pub use model::{ContentId, ContentType, Like};
pub use repository::{LikeCacheRepository, LikeDbRepository};
