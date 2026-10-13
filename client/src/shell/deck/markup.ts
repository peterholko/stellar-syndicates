import { accountBackdrop, accountFields, accountProfile } from "../account";

export function mountDeckMarkup(root: HTMLElement): void {
  root.innerHTML = `
    <div id="deck" class="deck">
      <header id="deck-topbar" class="deck-topbar">
        <div id="deck-identity" class="deck-identity"><b id="deck-corp">—</b><span id="deck-corp-id">—</span></div>
        <div id="deck-stats" class="deck-stats">
          <span>Credits <b id="deck-credits">—</b><small id="deck-reserved"></small></span>
          <span>Equity <b id="deck-equity">—</b></span>
          <span>Game <b id="deck-tick">—</b></span>
          <span id="deck-pacing"></span>
          <span id="deck-link">offline</span>
        </div>
        <div id="deck-construction-chip" class="deck-construction-chip" hidden></div>
        <nav id="deck-nav" class="deck-nav" aria-label="Command Deck">
          ${navButton("command", "Command")}
          ${navButton("fleets", "Fleets")}
          ${navButton("logistics", "Logistics")}
          ${navButton("market", "Market")}
          ${navButton("research", "Research")}
          ${navButton("officers", "Officers")}
          ${navButton("operations", "Operations")}
          ${navButton("syndicate", "Syndicate")}
          ${navButton("faction", "Faction")}
          ${navButton("log", "Log")}
          <button id="deck-nav-more" class="deck-nav__more" type="button" data-deck-act="nav-more" aria-expanded="false" aria-controls="deck-nav-overflow">More ···</button>
        </nav>
        <button id="deck-profile-open" type="button">Profile</button>
      </header>
      <div id="deck-construction-menu" class="deck-construction-menu" role="menu" aria-label="Construction across your worlds" hidden></div>
      <div id="deck-nav-overflow" class="deck-nav-overflow" role="menu" hidden>
        ${overflowButton("research", "Research")}
        ${overflowButton("officers", "Officers")}
        ${overflowButton("operations", "Operations")}
        ${overflowButton("syndicate", "Syndicate")}
        ${overflowButton("faction", "Faction")}
        ${overflowButton("log", "Log")}
        ${overflowButton("rankings", "Rankings")}
      </div>
      <section id="deck-planet-stage" class="deck-planet-stage" aria-label="Planet view" hidden>
        <div id="deck-planet-stage-body" class="deck-planet-stage__body"></div>
      </section>
      <aside id="deck-workspace" class="deck-workspace" aria-hidden="true">
        <header class="deck-workspace__header">
          <button type="button" data-deck-act="back" aria-label="Back">←</button>
          <nav id="deck-breadcrumb" class="deck-breadcrumb" aria-label="Breadcrumb"></nav>
          <h1 id="deck-workspace-title">Command</h1>
          <button type="button" data-deck-act="width" aria-label="Toggle workspace width">↔</button>
          <button type="button" data-deck-act="close" aria-label="Close workspace">✕</button>
        </header>
        <div id="deck-workspace-body" class="deck-workspace__body"></div>
      </aside>
      <section id="deck-build-workbench" class="deck-build-workbench" aria-labelledby="deck-build-workbench-title" tabindex="-1" hidden>
        <header class="deck-build-workbench__header">
          <div>
            <span>Construction workbench</span>
            <h2 id="deck-build-workbench-title">Build</h2>
          </div>
          <button type="button" data-deck-act="build-workbench-close" aria-label="Close construction workbench">✕</button>
        </header>
        <div id="deck-build-workbench-body" class="deck-build-workbench__body"></div>
      </section>
      <section id="deck-fleet-workbench" class="deck-build-workbench deck-fleet-workbench" aria-labelledby="deck-fleet-workbench-title" tabindex="-1" hidden>
        <header class="deck-build-workbench__header">
          <div>
            <span>Fleet details</span>
            <h2 id="deck-fleet-workbench-title">Fleet</h2>
          </div>
          <button type="button" data-deck-act="fleet-details-close" aria-label="Close fleet details">✕</button>
        </header>
        <div id="deck-fleet-workbench-body" class="deck-build-workbench__body deck-fleet-workbench__body"></div>
      </section>
      <aside id="deck-welcome" class="deck-welcome" aria-label="Welcome back report" hidden></aside>
      <section id="deck-toast-lane" class="deck-toast-lane" aria-live="polite"></section>
      <div id="deck-bottom-band" class="deck-bottom-band">
        <section id="deck-command-strip" class="deck-command-strip" aria-live="polite"></section>
        <aside id="deck-founding" class="deck-founding" hidden></aside>
        <div id="deck-zoom" class="deck-zoom" aria-label="Map zoom">
          <span id="deck-zoom-level" class="deck-zoom__level">1.0×</span>
          <button type="button" data-deck-act="zoom-in" aria-label="Zoom in">+</button>
          <button type="button" data-deck-act="zoom-fit" aria-label="Fit galaxy">⊡</button>
          <button type="button" data-deck-act="zoom-out" aria-label="Zoom out">−</button>
          <button type="button" data-deck-act="help" aria-label="Keyboard shortcuts">?</button>
        </div>
      </div>
      <div id="deck-hover" class="deck-hover" hidden></div>
      ${accountProfile("deck")}
      <div id="deck-overlays" class="deck-overlays">
        <section id="deck-join" class="deck-overlay deck-join" aria-labelledby="deck-join-title">
          ${accountBackdrop("deck")}
          <form id="deck-join-form" class="deck-join__card account-card" data-account-mode="signin">
            ${accountFields("deck")}
          </form>
        </section>
        <section id="deck-help" class="deck-overlay deck-help" aria-labelledby="deck-help-title" hidden>
          <div class="deck-help__card">
            <header><div><div class="deck-eyebrow">Command Deck</div><h1 id="deck-help-title">Shortcuts</h1></div><button type="button" data-deck-act="close-help" aria-label="Close shortcuts">✕</button></header>
            <div class="deck-help__grid">
              <kbd>Left-click</kbd><span>Select a map object, or command the selected fleet</span>
              <kbd>Shift-click</kbd><span>Attack a hostile target with the selected fleet</span>
              <kbd>Right-click</kbd><span>Inspect without disturbing fleet selection</span>
              <kbd>Double-click</kbd><span>Enter a star system or an ongoing battle</span>
              <kbd>Ctrl / ⌘-click</kbd><span>Add or remove an owned fleet from the command group</span>
              <kbd>Scroll · Drag</kbd><span>Zoom through semantic views · pan the galaxy</span>
              <kbd>+ · − · ⊡</kbd><span>Zoom in · zoom out · fit the galaxy</span>
              <kbd>Enter</kbd><span>Confirm the pending fleet order</span>
              <kbd>J / G</kbd><span>Arm Jump / Guard for the selected fleet</span>
              <kbd>[ / ]</kbd><span>Cycle and center your fleets</span>
              <kbd>M V T R P</kbd><span>Market · Fleets · Logistics · Research · Officers</span>
              <kbd>U Y C K L</kbd><span>Operations · Syndicate · Faction · Rankings · Log</span>
              <kbd>S</kbd><span>Open the selected system's workspace</span>
              <kbd>Esc</kbd><span>Cancel one layer: order · aim · scrub · overlay · workspace · selection</span>
              <kbd>?</kbd><span>Open or close this shortcuts overlay</span>
            </div>
            <p>Shortcuts accelerate controls that remain visible in the Command Deck.</p>
          </div>
        </section>
        <section id="deck-battle-theater" class="deck-overlay deck-theater-overlay" aria-label="Battle theater" hidden>
          <div class="deck-theater-backdrop" data-deck-theater-dismiss></div>
          <article id="deck-battle-theater-card" class="deck-theater-card deck-theater-card--battle"></article>
        </section>
        <section id="deck-ground-theater" class="deck-overlay deck-theater-overlay" aria-label="Ground theater" hidden>
          <div class="deck-theater-backdrop" data-deck-theater-dismiss></div>
          <article id="deck-ground-theater-card" class="deck-theater-card deck-theater-card--ground"></article>
        </section>
      </div>
    </div>`;
}

/** Top-bar destinations that fold into More first when the row runs out of
 * room; Command, Fleets, Logistics and Market always stay. */
const NAV_FOLD_ORDER = ["faction", "syndicate", "operations", "officers", "log", "research"];

/** Keep every destination reachable: when the row would clip (window width,
 * credit digits, the construction chip), fold the least-used destinations into
 * More and list exactly those there. */
export function fitDeckNav(): void {
  const nav = document.getElementById("deck-nav"), overflow = document.getElementById("deck-nav-overflow");
  if (!nav || !overflow) return;
  const button = (route: string) => nav.querySelector<HTMLButtonElement>(`:scope > [data-route="${route}"]`);
  nav.classList.remove("has-folds");
  for (const route of NAV_FOLD_ORDER) button(route)?.classList.remove("is-folded");
  for (const route of NAV_FOLD_ORDER) {
    if (nav.scrollWidth <= nav.clientWidth) break;
    nav.classList.add("has-folds");
    button(route)?.classList.add("is-folded");
  }
  for (const item of overflow.querySelectorAll<HTMLButtonElement>("[data-route]")) {
    const twin = button(item.dataset.route ?? "");
    item.hidden = !!twin && !twin.classList.contains("is-folded");
  }
  syncNavMoreBadge();
}

/** A folded destination's alert shows as a dot on More. */
export function syncNavMoreBadge(): void {
  const nav = document.getElementById("deck-nav");
  nav?.querySelector("#deck-nav-more")?.classList.toggle("has-badge", !!nav.querySelector(":scope > .is-folded .deck-nav__badge:not([hidden])"));
}

function navButton(route: string, text: string): string {
  return `<button type="button" data-deck-act="route" data-route="${route}">${text}<span id="deck-badge-${route}" class="deck-nav__badge" hidden></span></button>`;
}

function overflowButton(route: string, text: string): string {
  return `<button type="button" role="menuitem" data-deck-act="route" data-route="${route}">${text}</button>`;
}
