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
        <nav id="deck-nav" class="deck-nav" aria-label="Command Deck">
          ${navButton("command", "Command")}
          ${navButton("fleets", "Fleets")}
          ${navButton("market", "Market")}
          ${navButton("research", "Research")}
          ${navButton("officers", "Officers")}
          ${navButton("operations", "Operations")}
          ${navButton("syndicate", "Syndicate")}
          ${navButton("faction", "Faction")}
          ${navButton("log", "Log")}
          <button id="deck-nav-more" class="deck-nav__more" type="button" data-deck-act="nav-more" aria-expanded="false" aria-controls="deck-nav-overflow">More ···</button>
        </nav>
      </header>
      <div id="deck-nav-overflow" class="deck-nav-overflow" role="menu" hidden>
        ${overflowButton("rankings", "Rankings")}
        ${overflowButton("operations", "Operations")}
        ${overflowButton("syndicate", "Syndicate")}
        ${overflowButton("faction", "Faction")}
        ${overflowButton("log", "Log")}
      </div>
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
      <section id="deck-world-workbench" class="deck-build-workbench deck-world-workbench" aria-labelledby="deck-world-workbench-title" tabindex="-1" hidden>
        <header class="deck-build-workbench__header">
          <div class="deck-world-workbench__identity">
            <img id="deck-world-workbench-art" class="deck-world-workbench__art" alt="" hidden>
            <div>
              <span>World details</span>
              <h2 id="deck-world-workbench-title">World</h2>
            </div>
          </div>
          <button type="button" data-deck-act="world-workbench-close" aria-label="Close world details">✕</button>
        </header>
        <div id="deck-world-workbench-body" class="deck-build-workbench__body"></div>
      </section>
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
      <div id="deck-overlays" class="deck-overlays">
        <section id="deck-join" class="deck-overlay deck-join" aria-labelledby="deck-join-title">
          <form id="deck-join-form" class="deck-join__card">
            <div class="deck-eyebrow">Terran Charter Authority · corporate registry</div>
            <h1 id="deck-join-title">Establish your corporation</h1>
            <p>Your command picture is delayed by distance. Name the corporation whose light you will follow.</p>
            <label for="deck-name">Corporation name</label>
            <input id="deck-name" name="corporation" maxlength="32" autocomplete="organization" />
            <button id="deck-join-button" type="submit">Enter Command Deck</button>
            <div id="deck-join-error" class="deck-inline-error" role="alert"></div>
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
              <kbd>M V R P</kbd><span>Market · Fleets · Research · Officers</span>
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

function navButton(route: string, text: string): string {
  return `<button type="button" data-deck-act="route" data-route="${route}">${text}<span id="deck-badge-${route}" class="deck-nav__badge" hidden></span></button>`;
}

function overflowButton(route: string, text: string): string {
  return `<button type="button" role="menuitem" data-deck-act="route" data-route="${route}">${text}</button>`;
}
