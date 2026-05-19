-- Create role enum type for RBAC
CREATE TYPE user_role AS ENUM ('User', 'Admin', 'Owner');

-- Add role column to users table with default 'User' for new records
ALTER TABLE users ADD COLUMN role user_role NOT NULL DEFAULT 'User';

-- Create index for role lookups
CREATE INDEX idx_users_role ON users(role);

-- Create a view for checking owner status (for convenience in queries)
CREATE VIEW owner_users AS
  SELECT id, username, email FROM users WHERE role = 'Owner';
