#!/usr/bin/env python3
import sqlite3
import psycopg2
from psycopg2.extras import execute_values
import json
import binascii
import sys
import os

# Configuration
SQLITE_DB = os.getenv("SQLITE_DB", "helm-hub.db")
PG_DSN = os.getenv("DATABASE_URL", "postgres://postgres:postgres@localhost:5432/helm_hub")

def migrate():
    """
    Migrates data from SQLite to PostgreSQL for the helm-hub project.
    Handles schema changes, type conversions, and foreign key order.
    """
    if not os.path.exists(SQLITE_DB):
        print(f"Error: SQLite database file '{SQLITE_DB}' not found.")
        print("Set SQLITE_DB environment variable if your database is named differently.")
        sys.exit(1)

    print(f"Connecting to SQLite: {SQLITE_DB}")
    sqlite_conn = sqlite3.connect(SQLITE_DB)
    sqlite_conn.row_factory = sqlite3.Row
    sqlite_cur = sqlite_conn.cursor()

    print(f"Connecting to PostgreSQL: {PG_DSN}")
    try:
        pg_conn = psycopg2.connect(PG_DSN)
        pg_cur = pg_conn.cursor()
    except Exception as e:
        print(f"Error connecting to PostgreSQL: {e}")
        print("Ensure DATABASE_URL is set correctly.")
        sys.exit(1)

    try:
        # 1. Users
        print("Migrating users...")
        sqlite_cur.execute("SELECT * FROM users")
        users = sqlite_cur.fetchall()
        user_data = []
        for u in users:
            user_data.append((
                u['id'], u['username'], u['email'], u['password_hash'],
                u['totp_secret'], bool(u['totp_enabled']), bool(u['is_admin']),
                u['banned_at'], u.get('storage_usage_bytes', 0), u.get('storage_quota_bytes'),
                u['created_at'], u['updated_at']
            ))
        if user_data:
            execute_values(pg_cur, """
                INSERT INTO users (id, username, email, password_hash, totp_secret, totp_enabled, is_admin, banned_at, storage_usage_bytes, storage_quota_bytes, created_at, updated_at)
                VALUES %s ON CONFLICT (id) DO NOTHING
            """, user_data)
        print(f"Migrated {len(user_data)} users.")

        # 2. Artifacts (from charts)
        print("Migrating artifacts...")
        try:
            sqlite_cur.execute("SELECT * FROM charts")
            charts = sqlite_cur.fetchall()
            artifact_data = []
            for c in charts:
                metadata = {
                    "home_url": c.get('home_url'),
                    "icon_url": c.get('icon_url'),
                    "keywords": c.get('keywords').split(',') if c.get('keywords') else []
                }
                artifact_data.append((
                    c['id'], c['owner_id'], c['name'], 'helm', c.get('description'),
                    json.dumps(metadata), bool(c.get('is_private', 0)), c.get('download_count', 0),
                    c['created_at'], c['updated_at']
                ))
            if artifact_data:
                execute_values(pg_cur, """
                    INSERT INTO artifacts (id, owner_id, name, type, description, metadata, is_private, download_count, created_at, updated_at)
                    VALUES %s ON CONFLICT (id) DO NOTHING
                """, artifact_data)
            print(f"Migrated {len(artifact_data)} artifacts.")
        except sqlite3.OperationalError as e:
            print(f"Skipping artifacts (table 'charts' not found): {e}")

        # 3. Artifact Versions (from chart_versions)
        print("Migrating artifact versions...")
        try:
            sqlite_cur.execute("SELECT * FROM chart_versions")
            versions = sqlite_cur.fetchall()
            version_data = []
            for v in versions:
                metadata = {
                    "app_version": v.get('app_version'),
                    "description": v.get('description'),
                    "chart_yaml": v.get('chart_yaml'),
                    "values_yaml": v.get('values_yaml'),
                    "schema_json": v.get('schema_json')
                }
                # Decode hex digest to bytes
                digest_bytes = binascii.unhexlify(v['digest']) if v['digest'] else b''
                version_data.append((
                    v['id'], v['chart_id'], v['version'], digest_bytes,
                    0, # size was not in old schema
                    v['storage_path'], json.dumps(metadata), bool(v.get('deprecated', 0)),
                    v['created_at']
                ))
            if version_data:
                execute_values(pg_cur, """
                    INSERT INTO artifact_versions (id, artifact_id, version, digest, size, storage_path, metadata, deprecated, created_at)
                    VALUES %s ON CONFLICT (id) DO NOTHING
                """, version_data)
            print(f"Migrated {len(version_data)} artifact versions.")
        except sqlite3.OperationalError as e:
            print(f"Skipping artifact versions (table 'chart_versions' not found): {e}")

        # 4. API Tokens
        print("Migrating API tokens...")
        try:
            sqlite_cur.execute("SELECT * FROM api_tokens")
            tokens = sqlite_cur.fetchall()
            token_data = []
            for t in tokens:
                token_data.append((
                    t['id'], t['user_id'], t['description'], t['token_hash'],
                    t['expires_at'], t['last_used_at'], t['created_at']
                ))
            if token_data:
                execute_values(pg_cur, """
                    INSERT INTO api_tokens (id, user_id, description, token_hash, expires_at, last_used_at, created_at)
                    VALUES %s ON CONFLICT (id) DO NOTHING
                """, token_data)
            print(f"Migrated {len(token_data)} API tokens.")
        except sqlite3.OperationalError as e:
            print(f"Skipping API tokens: {e}")

        # 5. GitHub Connections
        print("Migrating GitHub connections...")
        try:
            sqlite_cur.execute("SELECT * FROM github_connections")
            conns = sqlite_cur.fetchall()
            conn_data = []
            for c in conns:
                conn_data.append((
                    c['id'], c['user_id'], c['github_id'], c['github_username'],
                    c['github_access_token'], c['avatar_url'], c['created_at'], c['updated_at']
                ))
            if conn_data:
                execute_values(pg_cur, """
                    INSERT INTO github_connections (id, user_id, github_id, github_username, github_access_token, avatar_url, created_at, updated_at)
                    VALUES %s ON CONFLICT (id) DO NOTHING
                """, conn_data)
            print(f"Migrated {len(conn_data)} GitHub connections.")
        except sqlite3.OperationalError as e:
            print(f"Skipping GitHub connections: {e}")

        # 6. GitHub Repos
        print("Migrating GitHub repos...")
        try:
            sqlite_cur.execute("SELECT * FROM github_repos")
            repos = sqlite_cur.fetchall()
            repo_data = []
            for r in repos:
                repo_data.append((
                    r['id'], r['user_id'], r['github_connection_id'],
                    r['repo_owner'], r['repo_name'], r['last_synced_at'], r['created_at']
                ))
            if repo_data:
                execute_values(pg_cur, """
                    INSERT INTO github_repos (id, user_id, github_connection_id, repo_owner, repo_name, last_synced_at, created_at)
                    VALUES %s ON CONFLICT (id) DO NOTHING
                """, repo_data)
            print(f"Migrated {len(repo_data)} GitHub repos.")
        except sqlite3.OperationalError as e:
            print(f"Skipping GitHub repos: {e}")

        # 7. Audit Logs (from admin_audit_log)
        print("Migrating audit logs...")
        try:
            sqlite_cur.execute("SELECT * FROM admin_audit_log")
            logs = sqlite_cur.fetchall()
            log_data = []
            for l in logs:
                log_data.append((
                    l['id'], l['admin_id'], l['action'], l['target_type'],
                    l['target_id'], l.get('metadata_json') or '{}', l['created_at']
                ))
            if log_data:
                execute_values(pg_cur, """
                    INSERT INTO audit_logs (id, actor_id, action, target_type, target_id, metadata, created_at)
                    VALUES %s ON CONFLICT (id, created_at) DO NOTHING
                """, log_data)
            print(f"Migrated {len(log_data)} audit logs.")
        except sqlite3.OperationalError as e:
            print(f"Skipping audit logs: {e}")

        # 8. App Settings
        print("Migrating app settings...")
        try:
            sqlite_cur.execute("SELECT * FROM app_settings")
            settings = sqlite_cur.fetchall()
            setting_data = []
            for s in settings:
                setting_data.append((
                    s['key'], s['value'], s['updated_at']
                ))
            if setting_data:
                execute_values(pg_cur, """
                    INSERT INTO app_settings (key, value, updated_at)
                    VALUES %s ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = EXCLUDED.updated_at
                """, setting_data)
            print(f"Migrated {len(setting_data)} app settings.")
        except sqlite3.OperationalError as e:
            print(f"Skipping app settings: {e}")

        # 9. Rate Limit Windows
        print("Migrating rate limit windows...")
        try:
            sqlite_cur.execute("SELECT * FROM rate_limit_windows")
            windows = sqlite_cur.fetchall()
            window_data = []
            for w in windows:
                window_data.append((
                    w['key'], w['count'], w['window_start']
                ))
            if window_data:
                execute_values(pg_cur, """
                    INSERT INTO rate_limit_windows (key, count, window_start)
                    VALUES %s ON CONFLICT (key) DO UPDATE SET count = EXCLUDED.count, window_start = EXCLUDED.window_start
                """, window_data)
            print(f"Migrated {len(window_data)} rate limit windows.")
        except sqlite3.OperationalError as e:
            print(f"Skipping rate limit windows: {e}")

        pg_conn.commit()
        print("\nMigration completed successfully!")

    except Exception as e:
        pg_conn.rollback()
        print(f"\nMigration failed: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
    finally:
        sqlite_conn.close()
        pg_conn.close()

if __name__ == "__main__":
    migrate()
