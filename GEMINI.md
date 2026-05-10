# Helm Hub Project Instructions

## Database Migrations

- **One Migration Per Table:** Each new table should be created in its own migration.
- **Immutability:** Once a migration has been committed and applied to production, it must be considered immutable. Do not modify existing migration files.
- **Schema Changes:** Any changes to existing tables (adding columns, changing types, adding indexes) must be performed in a new follow-up migration.
- **Foreign Keys:** Order migrations so that dependencies are created before the tables that reference them.
