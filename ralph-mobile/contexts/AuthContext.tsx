import {
  createContext,
  useContext,
  useState,
  useEffect,
  ReactNode,
} from "react";
import { useRouter, useSegments } from "expo-router";
import { api } from "@/lib/api";
import {
  saveToken,
  getToken,
  saveUser,
  getUser,
  clearAuth,
  User,
} from "@/lib/storage";

interface AuthContextType {
  user: User | null;
  isLoading: boolean;
  isAuthenticated: boolean;
  login: (email: string, password: string) => Promise<void>;
  register: (name: string, email: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
}

const AuthContext = createContext<AuthContextType | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const router = useRouter();
  const segments = useSegments();

  useEffect(() => {
    checkAuth();
  }, []);

  useEffect(() => {
    if (isLoading) return;

    const inAuthGroup = segments[0] === "(auth)";

    if (!user && !inAuthGroup) {
      // User is not signed in, redirect to login
      router.replace("/(auth)/login");
    } else if (user && inAuthGroup) {
      // User is signed in but on auth screen, redirect to home
      router.replace("/");
    }
  }, [user, segments, isLoading]);

  async function checkAuth() {
    try {
      const token = await getToken();
      if (!token) {
        setIsLoading(false);
        return;
      }

      // Try to load cached user first for faster UX
      const cachedUser = await getUser();
      if (cachedUser) {
        setUser(cachedUser);
      }

      // Verify token with server (for Ralph Orchestrator, we might skip this
      // if running locally without auth backend)
      try {
        const response = await api.get("/auth/me");
        setUser(response.data);
        await saveUser(response.data);
      } catch (error) {
        // If server auth fails but we have cached user, keep them logged in
        // This allows offline usage with previously cached credentials
        if (!cachedUser) {
          await clearAuth();
        }
      }
    } catch (error) {
      await clearAuth();
    } finally {
      setIsLoading(false);
    }
  }

  async function login(email: string, password: string) {
    const response = await api.post("/auth/login", { email, password });
    const { access_token, user: userData } = response.data;

    await saveToken(access_token);
    await saveUser(userData);
    setUser(userData);
  }

  async function register(name: string, email: string, password: string) {
    const response = await api.post("/auth/register", {
      name,
      email,
      password,
    });
    const { access_token, user: userData } = response.data;

    await saveToken(access_token);
    await saveUser(userData);
    setUser(userData);
  }

  async function logout() {
    await clearAuth();
    setUser(null);
  }

  return (
    <AuthContext.Provider
      value={{
        user,
        isLoading,
        isAuthenticated: !!user,
        login,
        register,
        logout,
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth must be used within AuthProvider");
  }
  return context;
}
