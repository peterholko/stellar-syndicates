export function mountDeckMarkup(root: HTMLElement): void {
  root.innerHTML = `
    <div id="deck" class="deck">
      <header id="deck-topbar" class="deck-topbar">
        <div id="deck-identity" class="deck-identity"><b id="deck-corp">—</b><span id="deck-corp-id">—</span></div>
        <div id="deck-stats" class="deck-stats">
          <span>Credits <b id="deck-credits">—</b><small id="deck-reserved"></small></span>
          <span>Equity <b id="deck-equity">—</b></span>
          <span>Tick <b id="deck-tick">—</b></span>
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
        </nav>
      </header>
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
      <section id="deck-command-strip" class="deck-command-strip" aria-live="polite"></section>
      <section id="deck-toast-lane" class="deck-toast-lane" aria-live="polite"></section>
      <aside id="deck-founding" class="deck-founding" hidden></aside>
      <div id="deck-zoom" class="deck-zoom" aria-label="Map zoom">
        <span id="deck-zoom-level" class="deck-zoom__level">1.0×</span>
        <button type="button" data-deck-act="zoom-in" aria-label="Zoom in">+</button>
        <button type="button" data-deck-act="zoom-fit" aria-label="Fit galaxy">⊡</button>
        <button type="button" data-deck-act="zoom-out" aria-label="Zoom out">−</button>
        <button type="button" data-deck-act="help" aria-label="Keyboard shortcuts">?</button>
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
              <kbd>Scroll</kbd><span>Zoom and cross the galaxy/system handoff</span>
              <kbd>Drag</kbd><span>Pan the galaxy map</span>
              <kbd>+</kbd><span>Zoom in</span>
              <kbd>−</kbd><span>Zoom out</span>
              <kbd>⊡</kbd><span>Fit the galaxy</span>
              <kbd>Esc</kbd><span>Back one layer</span>
            </div>
            <p>Fleet commands and their accelerators arrive with the D1 command grammar.</p>
          </div>
        </section>
      </div>
    </div>`;
}

function navButton(route: string, text: string): string {
  return `<button type="button" data-deck-act="route" data-route="${route}">${text}<span id="deck-badge-${route}" class="deck-nav__badge" hidden></span></button>`;
}
