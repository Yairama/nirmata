import { createContext, useContext, useSyncExternalStore } from "react";
import type { ReactNode } from "react";
import { getSessionSnapshot, subscribeSession } from "./state.js";
import type { WorldSession } from "./types.js";

const SessionContext = createContext<WorldSession | null>(null);

export function SessionProvider({ children }: { children: ReactNode }) {
  const session = useSyncExternalStore(subscribeSession, getSessionSnapshot);
  return <SessionContext.Provider value={session}>{children}</SessionContext.Provider>;
}

export function useSession(): WorldSession | null {
  return useContext(SessionContext);
}
