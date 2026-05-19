CREATE OR REPLACE FUNCTION public.get_public_artifact(
    p_owner_username TEXT,
    p_artifact_name TEXT
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
    updated_at TIMESTAMPTZ
)
LANGUAGE sql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
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
        a.updated_at
    FROM artifacts a
    JOIN users u ON u.id = a.owner_id
    WHERE u.username = p_owner_username
      AND a.name = p_artifact_name
      AND a.is_private = false
    LIMIT 1;
$$;

CREATE OR REPLACE FUNCTION public.list_public_artifact_versions(
    p_owner_username TEXT,
    p_artifact_name TEXT
)
RETURNS TABLE (
    id UUID,
    artifact_id UUID,
    version TEXT,
    digest BYTEA,
    size BIGINT,
    storage_path TEXT,
    metadata JSONB,
    deprecated BOOLEAN,
    created_at TIMESTAMPTZ
)
LANGUAGE sql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
    SELECT
        av.id,
        av.artifact_id,
        av.version,
        av.digest,
        av.size,
        av.storage_path,
        av.metadata,
        av.deprecated,
        av.created_at
    FROM artifact_versions av
    JOIN artifacts a ON a.id = av.artifact_id
    JOIN users u ON u.id = a.owner_id
    WHERE u.username = p_owner_username
      AND a.name = p_artifact_name
      AND a.is_private = false
    ORDER BY av.created_at DESC;
$$;

CREATE OR REPLACE FUNCTION public.get_public_artifact_version(
    p_owner_username TEXT,
    p_artifact_name TEXT,
    p_version TEXT
)
RETURNS TABLE (
    id UUID,
    artifact_id UUID,
    version TEXT,
    digest BYTEA,
    size BIGINT,
    storage_path TEXT,
    metadata JSONB,
    deprecated BOOLEAN,
    created_at TIMESTAMPTZ
)
LANGUAGE sql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
    SELECT
        av.id,
        av.artifact_id,
        av.version,
        av.digest,
        av.size,
        av.storage_path,
        av.metadata,
        av.deprecated,
        av.created_at
    FROM artifact_versions av
    JOIN artifacts a ON a.id = av.artifact_id
    JOIN users u ON u.id = a.owner_id
    WHERE u.username = p_owner_username
      AND a.name = p_artifact_name
      AND av.version = p_version
      AND a.is_private = false
    LIMIT 1;
$$;
