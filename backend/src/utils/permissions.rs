use uuid::Uuid;
use crate::db::user_role::UserRole;

/// Checks if an actor can promote another user to a target role.
/// 
/// Rules:
/// - Owner can promote to any role
/// - Admin can promote User to Admin (but not to Owner)
/// - User and lower cannot promote anyone
pub fn can_promote_to(actor_role: UserRole, target_role: UserRole) -> bool {
    match actor_role {
        UserRole::Owner => true,
        UserRole::Admin => {
            // Admin can promote to User or Admin, but not Owner
            !matches!(target_role, UserRole::Owner)
        }
        UserRole::User => false,
    }
}

/// Checks if an actor can delete a user.
/// 
/// Rules:
/// - Owner can delete any user
/// - Admin can delete non-Owner users
/// - User cannot delete anyone (even themselves via admin endpoint)
pub fn can_delete_user(actor_role: UserRole) -> bool {
    matches!(actor_role, UserRole::Owner | UserRole::Admin)
}

/// Checks if an actor can delete a specific chart.
/// 
/// Rules:
/// - Owner can delete any chart
/// - Admin can delete any chart
/// - User can only delete their own chart
pub fn can_delete_chart(actor_role: UserRole, chart_owner_id: Uuid, actor_id: Uuid) -> bool {
    match actor_role {
        UserRole::Owner | UserRole::Admin => true,
        UserRole::User => actor_id == chart_owner_id,
    }
}

/// Checks if an actor can view analytics.
/// 
/// Rules:
/// - Owner can view all analytics
/// - Admin can view instance-wide analytics (analytics_owner_id = None)
/// - User can only view their own analytics
pub fn can_view_analytics(
    actor_role: UserRole,
    analytics_owner_id: Option<Uuid>,
    actor_id: Uuid,
) -> bool {
    match actor_role {
        UserRole::Owner => true,
        UserRole::Admin => {
            // Admin can view instance-wide analytics
            analytics_owner_id.is_none()
        }
        UserRole::User => {
            // User can only view their own analytics
            analytics_owner_id.map(|owner_id| owner_id == actor_id).unwrap_or(false)
        }
    }
}

/// Checks if a role has admin-level permissions or higher.
/// 
/// This is a convenience function wrapping UserRole::is_admin_or_higher.
pub fn is_admin_or_higher(role: UserRole) -> bool {
    role.is_admin_or_higher()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Tests for can_promote_to
    #[test]
    fn test_owner_can_promote_to_any_role() {
        assert!(can_promote_to(UserRole::Owner, UserRole::User));
        assert!(can_promote_to(UserRole::Owner, UserRole::Admin));
        assert!(can_promote_to(UserRole::Owner, UserRole::Owner));
    }

    #[test]
    fn test_admin_can_promote_to_user_and_admin() {
        assert!(can_promote_to(UserRole::Admin, UserRole::User));
        assert!(can_promote_to(UserRole::Admin, UserRole::Admin));
    }

    #[test]
    fn test_admin_cannot_promote_to_owner() {
        assert!(!can_promote_to(UserRole::Admin, UserRole::Owner));
    }

    #[test]
    fn test_user_cannot_promote() {
        assert!(!can_promote_to(UserRole::User, UserRole::User));
        assert!(!can_promote_to(UserRole::User, UserRole::Admin));
        assert!(!can_promote_to(UserRole::User, UserRole::Owner));
    }

    // Tests for can_delete_user
    #[test]
    fn test_owner_can_delete_user() {
        assert!(can_delete_user(UserRole::Owner));
    }

    #[test]
    fn test_admin_can_delete_user() {
        assert!(can_delete_user(UserRole::Admin));
    }

    #[test]
    fn test_user_cannot_delete_user() {
        assert!(!can_delete_user(UserRole::User));
    }

    // Tests for can_delete_chart
    #[test]
    fn test_owner_can_delete_any_chart() {
        let owner_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        assert!(can_delete_chart(UserRole::Owner, owner_id, actor_id));
    }

    #[test]
    fn test_admin_can_delete_any_chart() {
        let owner_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        assert!(can_delete_chart(UserRole::Admin, owner_id, actor_id));
    }

    #[test]
    fn test_user_can_delete_own_chart() {
        let id = Uuid::new_v4();
        assert!(can_delete_chart(UserRole::User, id, id));
    }

    #[test]
    fn test_user_cannot_delete_others_chart() {
        let chart_owner_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        assert!(!can_delete_chart(UserRole::User, chart_owner_id, actor_id));
    }

    // Tests for can_view_analytics
    #[test]
    fn test_owner_can_view_all_analytics() {
        let actor_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        assert!(can_view_analytics(UserRole::Owner, None, actor_id));
        assert!(can_view_analytics(UserRole::Owner, Some(owner_id), actor_id));
    }

    #[test]
    fn test_admin_can_view_instance_analytics() {
        let actor_id = Uuid::new_v4();
        assert!(can_view_analytics(UserRole::Admin, None, actor_id));
    }

    #[test]
    fn test_admin_cannot_view_user_analytics() {
        let actor_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        assert!(!can_view_analytics(UserRole::Admin, Some(owner_id), actor_id));
    }

    #[test]
    fn test_user_can_view_own_analytics() {
        let id = Uuid::new_v4();
        assert!(can_view_analytics(UserRole::User, Some(id), id));
    }

    #[test]
    fn test_user_cannot_view_others_analytics() {
        let actor_id = Uuid::new_v4();
        let owner_id = Uuid::new_v4();
        assert!(!can_view_analytics(UserRole::User, Some(owner_id), actor_id));
    }

    #[test]
    fn test_user_cannot_view_instance_analytics() {
        let actor_id = Uuid::new_v4();
        assert!(!can_view_analytics(UserRole::User, None, actor_id));
    }

    // Tests for is_admin_or_higher
    #[test]
    fn test_is_admin_or_higher() {
        assert!(is_admin_or_higher(UserRole::Owner));
        assert!(is_admin_or_higher(UserRole::Admin));
        assert!(!is_admin_or_higher(UserRole::User));
    }
}
