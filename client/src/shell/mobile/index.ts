import type { Shell } from "../types";

// Phase 4 replaces this boundary marker with the portrait shell. Keeping the
// branch resolvable now lets Vite prove that boot selects shells dynamically.
export function createShell(): Shell {
  throw new Error("the mobile shell lands in Phase 4");
}
