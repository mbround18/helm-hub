# SQLite to PostgreSQL Migration Guide

This document outlines the steps to migrate the Helm Hub database from SQLite to PostgreSQL.

## Prerequisites

- Python 3.x
- `psycopg2` or `psycopg2-binary` installed (`pip install psycopg2-binary`)
- PostgreSQL server installed and running
- `diesel-cli` installed

## Migration Steps

### 1. Back up SQLite Database

Before starting, ensure you have a backup of your `helm-hub.db` file.

```bash
cp helm-hub.db helm-hub.db.bak
```

### 2. Set up PostgreSQL

Create a new PostgreSQL database for Helm Hub.

```sql
CREATE DATABASE helm_hub;
```

### 3. Run Diesel Migrations

Initialize the PostgreSQL schema using Diesel.

```bash
cd backend
export DATABASE_URL=postgres://user:password@localhost:5432/helm_hub
diesel migration run
```

### 4. Run the Migration Script

Use the provided Python script to migrate data from SQLite to PostgreSQL.

```bash
# From the project root
export DATABASE_URL=postgres://user:password@localhost:5432/helm_hub
export SQLITE_DB=helm-hub.db
python3 scripts/migrate_sqlite_to_pg.py
```

### 5. Update Application Configuration

Update your `.env` file or environment variables to use the new PostgreSQL connection string.

```env
DATABASE_URL=postgres://user:password@localhost:5432/helm_hub
```

### 6. Restart the Application

Restart the Helm Hub backend to start using PostgreSQL.

## Zero-Downtime Strategy (Recommended)

To minimize downtime during migration:

1. **Maintenance Mode**: Enable maintenance mode or stop writes to the SQLite database.
2. **Data Migration**: Run the migration script as described above.
3. **Verification**: Verify that the data has been correctly migrated to PostgreSQL.
4. **Switch Traffic**: Update the `DATABASE_URL` and restart the backend services.
5. **Post-Migration**: Monitor the logs for any issues and eventually decommission the SQLite database.

## Troubleshooting

- **Foreign Key Violations**: The migration script follows a specific order (Users -> Artifacts -> Versions) to avoid foreign key issues. If you encounter errors, ensure the PostgreSQL schema is clean before running the script.
- **Type Mismatches**: The script handles common conversions (UUIDs, Booleans, Timestamps, JSONB). If you have custom data types, you may need to adjust the script.
- **Missing Tables**: If some tables are missing in your SQLite database, the script will skip them with a warning.
