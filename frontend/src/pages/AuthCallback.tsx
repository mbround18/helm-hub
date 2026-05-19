import { useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { authApi } from "../lib/api";
import { useAuthStore } from "../stores/auth";

export function AuthCallback() {
  const [error, setError] = useState("");
  const navigate = useNavigate();
  const location = useLocation();
  const setAuth = useAuthStore((s) => s.setAuth);
  const clearAuth = useAuthStore((s) => s.clearAuth);

  useEffect(() => {
    const params = new URLSearchParams(location.search);
    const token = params.get("token");
    const returnTo = params.get("return_to") || "/profile";
    const failure = params.get("error");

    if (failure) {
      clearAuth();
      setError("Sign-in failed");
      return;
    }
    if (!token) {
      clearAuth();
      setError("Missing auth token");
      return;
    }

    (async () => {
      localStorage.setItem("token", token);
      try {
        const { data } = await authApi.me();
        setAuth(token, data);
        navigate(returnTo, { replace: true });
      } catch {
        clearAuth();
        setError("Unable to complete sign-in");
      }
    })();
  }, [clearAuth, location.search, navigate, setAuth]);

  return (
    <div className="min-h-screen bg-gray-950 flex items-center justify-center p-4">
      <div className="w-full max-w-sm rounded-xl border border-gray-800 bg-gray-900 p-6 text-center">
        <div className="text-sm text-gray-400">
          {error || "Finishing sign-in…"}
        </div>
      </div>
    </div>
  );
}
