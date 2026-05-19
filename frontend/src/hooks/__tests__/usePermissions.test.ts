import { describe, test, expect, beforeEach, vi } from "vitest";
import { usePermissions } from "../usePermissions";
import * as authStore from "../../stores/auth";

// Mock the auth store
vi.mock("../../stores/auth", () => ({
  useAuthStore: vi.fn(),
}));

describe("usePermissions", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Guest user (no authentication)", () => {
    test("correctly identifies Guest role when user is null", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: null,
        token: null,
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.isGuest).toBe(true);
      expect(permissions.isUser).toBe(false);
      expect(permissions.isAdmin).toBe(false);
      expect(permissions.isOwner).toBe(false);
    });

    test("guest cannot promote users", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: null,
        token: null,
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canPromoteUsers).toBe(false);
    });

    test("guest cannot delete anything", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: null,
        token: null,
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canDeleteAnyChart).toBe(false);
      expect(permissions.canDeleteUsers).toBe(false);
      expect(permissions.canDeleteOwnCharts).toBe(false);
    });

    test("guest cannot view instance analytics", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: null,
        token: null,
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canViewInstanceAnalytics).toBe(false);
    });

    test("guest cannot view package analytics", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: null,
        token: null,
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canViewPackageAnalytics("some-owner")).toBe(false);
    });
  });

  describe("Role Detection", () => {
    test("correctly identifies Owner role", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-1",
          username: "owner-user",
          email: "owner@example.com",
          role: "Owner",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.isOwner).toBe(true);
      expect(permissions.isAdmin).toBe(true);
      expect(permissions.isUser).toBe(false);
      expect(permissions.isGuest).toBe(false);
    });

    test("correctly identifies Admin role", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-2",
          username: "admin-user",
          email: "admin@example.com",
          role: "Admin",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.isAdmin).toBe(true);
      expect(permissions.isOwner).toBe(false);
      expect(permissions.isUser).toBe(false);
      expect(permissions.isGuest).toBe(false);
    });

    test("correctly identifies User role", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-3",
          username: "regular-user",
          email: "user@example.com",
          role: "User",
          is_admin: false,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.isUser).toBe(true);
      expect(permissions.isAdmin).toBe(false);
      expect(permissions.isOwner).toBe(false);
      expect(permissions.isGuest).toBe(false);
    });
  });

  describe("Promotion Permissions", () => {
    test("Owner can promote any user", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "owner-1",
          username: "owner-user",
          email: "owner@example.com",
          role: "Owner",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canPromoteUsers).toBe(true);
    });

    test("Admin can promote users", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "admin-1",
          username: "admin-user",
          email: "admin@example.com",
          role: "Admin",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canPromoteUsers).toBe(true);
    });

    test("User cannot promote anyone", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-1",
          username: "regular-user",
          email: "user@example.com",
          role: "User",
          is_admin: false,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canPromoteUsers).toBe(false);
    });
  });

  describe("Deletion Permissions", () => {
    test("Owner can delete any chart", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "owner-1",
          username: "owner-user",
          email: "owner@example.com",
          role: "Owner",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canDeleteAnyChart).toBe(true);
    });

    test("Admin can delete any chart", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "admin-1",
          username: "admin-user",
          email: "admin@example.com",
          role: "Admin",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canDeleteAnyChart).toBe(true);
    });

    test("User can delete only own charts", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-1",
          username: "regular-user",
          email: "user@example.com",
          role: "User",
          is_admin: false,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canDeleteAnyChart).toBe(false);
      expect(permissions.canDeleteOwnCharts).toBe(true);
    });

    test("Guest cannot delete anything", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: null,
        token: null,
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canDeleteAnyChart).toBe(false);
      expect(permissions.canDeleteOwnCharts).toBe(false);
    });
  });

  describe("Analytics Permissions", () => {
    test("Owner can view instance analytics", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "owner-1",
          username: "owner-user",
          email: "owner@example.com",
          role: "Owner",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canViewInstanceAnalytics).toBe(true);
    });

    test("Admin can view instance analytics", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "admin-1",
          username: "admin-user",
          email: "admin@example.com",
          role: "Admin",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canViewInstanceAnalytics).toBe(true);
    });

    test("User cannot view instance analytics", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-1",
          username: "regular-user",
          email: "user@example.com",
          role: "User",
          is_admin: false,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canViewInstanceAnalytics).toBe(false);
    });

    test("User can view own package analytics", () => {
      const userId = "user-1";
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: userId,
          username: "regular-user",
          email: "user@example.com",
          role: "User",
          is_admin: false,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canViewPackageAnalytics(userId)).toBe(true);
    });

    test("User cannot view other package analytics", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-1",
          username: "regular-user",
          email: "user@example.com",
          role: "User",
          is_admin: false,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canViewPackageAnalytics("other-user-id")).toBe(false);
    });

    test("Admin can view any package analytics", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "admin-1",
          username: "admin-user",
          email: "admin@example.com",
          role: "Admin",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canViewPackageAnalytics("any-owner-id")).toBe(true);
    });
  });

  describe("Edge Cases", () => {
    test("handles missing role gracefully", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-1",
          username: "user",
          email: "user@example.com",
          // role is undefined
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      // Should treat as User role by default or handle gracefully
      expect(permissions.isGuest).toBe(false);
    });

    test("handles empty user object", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {},
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      // Should not crash and handle gracefully
      expect(permissions).toBeDefined();
    });

    test("correctly handles role changes", () => {
      // First render as User
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-1",
          username: "user",
          email: "user@example.com",
          role: "User",
          is_admin: false,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      let permissions = usePermissions();
      expect(permissions.isUser).toBe(true);
      expect(permissions.isAdmin).toBe(false);

      // Simulate role change to Admin
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-1",
          username: "user",
          email: "user@example.com",
          role: "Admin",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      permissions = usePermissions();
      expect(permissions.isUser).toBe(false);
      expect(permissions.isAdmin).toBe(true);
    });
  });

  describe("User Management Permissions", () => {
    test("Admin can manage users", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "admin-1",
          username: "admin-user",
          email: "admin@example.com",
          role: "Admin",
          is_admin: true,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canManageUser("any-user-id")).toBe(true);
    });

    test("Regular user cannot manage other users", () => {
      vi.mocked(authStore.useAuthStore).mockReturnValue({
        user: {
          id: "user-1",
          username: "regular-user",
          email: "user@example.com",
          role: "User",
          is_admin: false,
        },
        token: "token",
        login: vi.fn(),
        logout: vi.fn(),
        setUser: vi.fn(),
      } as any);

      const permissions = usePermissions();
      expect(permissions.canManageUser("other-user-id")).toBe(false);
    });
  });
});
