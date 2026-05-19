CREATE OR REPLACE FUNCTION public.list_public_artifacts(
    p_query TEXT DEFAULT NULL,
    p_limit BIGINT DEFAULT 20,
    p_offset BIGINT DEFAULT 0
)
RETURNS TABLE (
    id UUID,
    owner_id UUID,
    name TEXT,
    type TEXT,
    description TEXT,
    metadata JSONB,
    is_private BOOLEAN,
    download_count INT,
    created_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ,
    owner_username TEXT
)
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
DECLARE
    v_query TEXT := NULLIF(BTRIM(COALESCE(p_query, '')), '');
    v_limit BIGINT := LEAST(GREATEST(COALESCE(p_limit, 20), 1), 100);
    v_offset BIGINT := GREATEST(COALESCE(p_offset, 0), 0);
BEGIN
    IF v_query IS NULL THEN
        RETURN QUERY
        SELECT
            a.id,
            a.owner_id,
            a.name,
            a.type,
            a.description,
            a.metadata,
            a.is_private,
            a.download_count,
            a.created_at,
            a.updated_at,
            u.username::TEXT
        FROM artifacts a
        JOIN users u ON u.id = a.owner_id
        WHERE a.is_private = false
        ORDER BY a.download_count DESC
        LIMIT v_limit OFFSET v_offset;
    ELSE
        RETURN QUERY
        SELECT
            a.id,
            a.owner_id,
            a.name,
            a.type,
            a.description,
            a.metadata,
            a.is_private,
            a.download_count,
            a.created_at,
            a.updated_at,
            u.username::TEXT
        FROM artifacts a
        JOIN users u ON u.id = a.owner_id
        WHERE a.is_private = false
          AND (a.name % v_query OR COALESCE(a.description, '') % v_query)
        ORDER BY similarity(a.name, v_query) DESC
        LIMIT v_limit OFFSET v_offset;
    END IF;
END;
$$;
