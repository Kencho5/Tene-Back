CREATE TABLE blogs (
    id            SERIAL PRIMARY KEY,
    title         VARCHAR(255) NOT NULL,
    slug          VARCHAR(280) NOT NULL UNIQUE,
    excerpt       TEXT,
    content       TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'draft'
                  CHECK (status IN ('draft', 'published')),
    published_at  TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_blogs_status ON blogs(status);
CREATE INDEX idx_blogs_published_at ON blogs(published_at DESC);
CREATE INDEX idx_blogs_created_at ON blogs(created_at DESC);

CREATE TABLE blog_media (
    id            SERIAL PRIMARY KEY,
    blog_id       INTEGER NOT NULL REFERENCES blogs(id) ON DELETE CASCADE,
    media_uuid    UUID NOT NULL UNIQUE DEFAULT gen_random_uuid(),
    media_type    TEXT NOT NULL
                  CHECK (media_type IN ('image', 'video')),
    extension     VARCHAR(10) NOT NULL,
    is_thumbnail  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_blog_media_blog_id ON blog_media(blog_id);

CREATE UNIQUE INDEX idx_blog_media_one_thumbnail
    ON blog_media(blog_id)
    WHERE is_thumbnail;
