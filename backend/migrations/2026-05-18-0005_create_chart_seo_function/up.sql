CREATE SCHEMA IF NOT EXISTS seo;

CREATE OR REPLACE FUNCTION seo.chart_seo(
    p_owner_username TEXT,
    p_chart_name TEXT
)
RETURNS TABLE (
    app_name TEXT,
    title TEXT,
    description TEXT,
    canonical_path TEXT
)
LANGUAGE sql
SECURITY DEFINER
SET search_path = public, pg_temp
AS $$
    WITH app AS (
        SELECT COALESCE(
            (SELECT value FROM app_settings WHERE key = 'app_name' LIMIT 1),
            'Helm Hub'
        ) AS app_name
    ),
    chart AS (
        SELECT
            a.name,
            a.description,
            u.username AS owner_username
        FROM artifacts a
        JOIN users u ON u.id = a.owner_id
        WHERE u.username = p_owner_username
          AND a.name = p_chart_name
          AND a.is_private = false
        LIMIT 1
    )
    SELECT
        app.app_name,
        CASE
            WHEN chart.name IS NOT NULL THEN format('%s by %s | %s', chart.name, chart.owner_username, app.app_name)
            ELSE format('%s by %s | %s', p_chart_name, p_owner_username, app.app_name)
        END AS title,
        COALESCE(
            chart.description,
            format('Helm chart %s by %s', p_chart_name, p_owner_username)
        ) AS description,
        format('/charts/%s/%s', p_owner_username, p_chart_name) AS canonical_path
    FROM app
    LEFT JOIN chart ON true;
$$;
