CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

CREATE TABLE IF NOT EXISTS likes (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    user_id UUID NOT NULL,
    content_type TEXT NOT NULL,
    content_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    -- Garantisce un solo like per utente per contenuto 
    CONSTRAINT unique_user_content_like UNIQUE(user_id, content_type, content_id)
);

-- Indici per query efficienti [cite: 22, 23]
CREATE INDEX idx_likes_content_count ON likes (content_type, content_id);
CREATE INDEX idx_likes_user_items ON likes (user_id, created_at DESC);
CREATE INDEX idx_likes_leaderboard ON likes (content_type, created_at);
