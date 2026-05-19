use diesel::{
    deserialize::FromSql,
    pg::Pg,
    serialize::ToSql,
    sql_types::Text,
    expression::AsExpression,
    backend::Backend,
    deserialize::{self, Queryable},
};
use std::io::Write;

/// User role in the RBAC system.
/// 
/// Hierarchy: Owner > Admin > User
/// - Owner: Bootstrap admin, highest privilege, cannot be removed
/// - Admin: Can manage app settings, users (except Owners)
/// - User: Default role for regular users
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
    serde::Serialize, serde::Deserialize,
    AsExpression,
)]
#[diesel(sql_type = Text)]
pub enum UserRole {
    User,
    Admin,
    Owner,
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::User => "User",
            UserRole::Admin => "Admin",
            UserRole::Owner => "Owner",
        }
    }

    /// Parse a role from a string. Note: This is a custom method, not `std::str::FromStr`.
    /// Use `UserRole::parse()` or implement `FromStr` if needed for `parse()` syntax.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "User" => Some(UserRole::User),
            "Admin" => Some(UserRole::Admin),
            "Owner" => Some(UserRole::Owner),
            _ => None,
        }
    }

    /// Check if this role has admin-level permissions or higher.
    pub fn is_admin_or_higher(&self) -> bool {
        matches!(self, UserRole::Admin | UserRole::Owner)
    }

    /// Check if this role is the Owner (highest privilege).
    pub fn is_owner(&self) -> bool {
        matches!(self, UserRole::Owner)
    }
}

// Diesel SQL type support
impl ToSql<Text, Pg> for UserRole {
    fn to_sql<'b>(&'b self, out: &mut diesel::serialize::Output<'b, '_, Pg>) -> diesel::serialize::Result {
        out.write_all(self.as_str().as_bytes())?;
        Ok(diesel::serialize::IsNull::No)
    }
}

impl FromSql<Text, Pg> for UserRole {
    fn from_sql(bytes: <Pg as Backend>::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        let s = std::str::from_utf8(bytes.as_bytes())?;
        UserRole::parse(s).ok_or_else(|| format!("Invalid role: {}", s).into())
    }
}

// Implement Queryable for UserRole
impl<DB> Queryable<Text, DB> for UserRole
where
    DB: Backend,
    String: FromSql<Text, DB>,
{
    type Row = String;

    fn build(row: Self::Row) -> deserialize::Result<Self> {
        UserRole::parse(&row).ok_or_else(|| format!("Invalid role: {}", row).into())
    }
}
