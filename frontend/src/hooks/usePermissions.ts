import { useAuthStore } from "../stores/auth";

interface Permissions {
  isOwner: boolean;
  isAdmin: boolean;
  isUser: boolean;
  isGuest: boolean;
  canPromoteUsers: boolean;
  canDeleteUsers: boolean;
  canDeleteAnyChart: boolean;
  canDeleteOwnCharts: boolean;
  canViewInstanceAnalytics: boolean;
  canViewPackageAnalytics: (ownerId: string) => boolean;
  canManageUser: (targetUserId: string) => boolean;
}

export function usePermissions(): Permissions {
  const { user } = useAuthStore();

  if (!user) {
    return {
      isOwner: false,
      isAdmin: false,
      isUser: false,
      isGuest: true,
      canPromoteUsers: false,
      canDeleteUsers: false,
      canDeleteAnyChart: false,
      canDeleteOwnCharts: false,
      canViewInstanceAnalytics: false,
      canViewPackageAnalytics: () => false,
      canManageUser: () => false,
    };
  }

  // Parse role from user object (will come from JWT)
  const role = user.role || (user.is_admin ? "Admin" : "User");
  const isOwner = role === "Owner";
  const isAdmin = role === "Admin" || isOwner;
  const isGuest = false;
  const isUser = !isGuest && !isAdmin;

  return {
    isOwner,
    isAdmin,
    isUser,
    isGuest,
    canPromoteUsers: isAdmin,
    canDeleteUsers: isAdmin,
    canDeleteAnyChart: isAdmin,
    canDeleteOwnCharts: !isGuest,
    canViewInstanceAnalytics: isAdmin,
    canViewPackageAnalytics: (ownerId: string) => isAdmin || user.id === ownerId,
    canManageUser: () =>
      isAdmin,
  };
}
