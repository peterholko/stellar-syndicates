// One physical desktop workspace. Feature modules still own their page content,
// but none of them owns placement, z-order, or the back stack anymore.

export const WORKSPACE_PAGE_IDS = [
  "rail", "ship-panel", "hub-panel", "battle-panel", "market",
  "research-panel", "operations-panel", "syndicate-panel", "faction-panel", "checkin",
] as const;

export type WorkspacePageId = typeof WORKSPACE_PAGE_IDS[number];

const PAGE_TITLE: Record<WorkspacePageId, string> = {
  rail: "Command workspace",
  "ship-panel": "Fleet detail",
  "hub-panel": "Wormhole Hub",
  "battle-panel": "Battle report",
  market: "Market Hub",
  "research-panel": "Research",
  "operations-panel": "Operations",
  "syndicate-panel": "Syndicate",
  "faction-panel": "Authority charter",
  checkin: "Decision inbox",
};

const PAGE_NAV: Partial<Record<WorkspacePageId, string>> = {
  market: "nav-market",
  "research-panel": "nav-research",
  "operations-panel": "nav-operations",
  "syndicate-panel": "nav-syndicate",
  "faction-panel": "nav-faction",
  checkin: "nav-log",
};

let initialized = false;
let active: WorkspacePageId | null = null;
let history: WorkspacePageId[] = [];
let refreshPage: (page: WorkspacePageId) => void = () => {};

const byId = (id: string): HTMLElement => document.getElementById(id)!;

function setPageVisible(id: WorkspacePageId, visible: boolean): void {
  const page = byId(id);
  page.classList.toggle("is-open", visible);
  if (id === "checkin") page.style.display = visible ? "block" : "none";
}

function syncChrome(): void {
  const root = byId("desktop-workspace");
  root.classList.toggle("is-open", active !== null);
  root.setAttribute("aria-hidden", active === null ? "true" : "false");
  const back = byId("workspace-back") as HTMLButtonElement;
  back.disabled = history.length === 0;
  back.classList.toggle("is-visible", history.length > 0);
  byId("workspace-title").textContent = active ? PAGE_TITLE[active] : "Command workspace";

  for (const nav of document.querySelectorAll<HTMLElement>(".hud-nav .hud-btn")) {
    nav.classList.remove("is-active");
  }
  if (active) {
    const navId = PAGE_NAV[active];
    if (navId) byId(navId).classList.add("is-active");
  }
}

function show(page: WorkspacePageId): void {
  for (const id of WORKSPACE_PAGE_IDS) setPageVisible(id, id === page);
  active = page;
  syncChrome();
  refreshPage(page);
}

export function initDesktopWorkspace(onRefresh: (page: WorkspacePageId) => void): void {
  refreshPage = onRefresh;
  if (initialized) return;
  initialized = true;
  const pages = byId("workspace-pages");
  for (const id of WORKSPACE_PAGE_IDS) {
    const page = byId(id);
    setPageVisible(id, false);
    pages.append(page);
  }
  byId("workspace-back").addEventListener("click", workspaceBack);
  byId("workspace-close").addEventListener("click", closeDesktopWorkspace);
  syncChrome();
}

export function activateWorkspacePage(page: WorkspacePageId, push = true): void {
  if (active === page) {
    refreshPage(page);
    return;
  }
  if (push && active) {
    history = history.filter((entry) => entry !== active);
    history.push(active);
  }
  show(page);
}

export function deactivateWorkspacePage(page: WorkspacePageId): void {
  if (active !== page) {
    setPageVisible(page, false);
    return;
  }
  workspaceBack();
}

export function workspaceBack(): void {
  if (active) setPageVisible(active, false);
  const previous = history.pop() ?? null;
  if (previous) show(previous);
  else {
    active = null;
    syncChrome();
  }
}

export function closeDesktopWorkspace(): void {
  for (const id of WORKSPACE_PAGE_IDS) setPageVisible(id, false);
  active = null;
  history = [];
  syncChrome();
}

export function workspacePageIsActive(page: WorkspacePageId): boolean {
  return active === page;
}

export function activeWorkspacePage(): WorkspacePageId | null {
  return active;
}

export function setWorkspaceTitle(title: string): void {
  if (active) byId("workspace-title").textContent = title;
}
