export function mountDeckMarkup(root: HTMLElement): void {
  root.innerHTML = `
    <div id="deck" class="deck">
      <header id="deck-topbar" class="deck-topbar">
        <div id="deck-identity" class="deck-identity"></div>
        <div id="deck-stats" class="deck-stats"></div>
        <nav id="deck-nav" class="deck-nav" aria-label="Command Deck"></nav>
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
      <div id="deck-zoom" class="deck-zoom"></div>
      <div id="deck-hover" class="deck-hover" hidden></div>
      <div id="deck-overlays" class="deck-overlays"></div>
    </div>`;
}
