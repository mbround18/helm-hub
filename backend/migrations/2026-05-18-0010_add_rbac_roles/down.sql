-- Drop the owner_users view first (views that depend on columns must be dropped)
DROP VIEW IF EXISTS owner_users;

-- Drop role index
DROP INDEX IF EXISTS idx_users_role;

-- Remove role column
ALTER TABLE users DROP COLUMN IF EXISTS role;

-- Drop the enum type
DROP TYPE IF EXISTS user_role;
