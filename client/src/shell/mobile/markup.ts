export const mobileMarkup = String.raw`
  <div class="m-shell">
    <header id="m-chrome" class="m-chrome" hidden>
      <button id="m-status-toggle" class="m-status" type="button" aria-expanded="false" aria-controls="m-status-more">
        <span class="m-stat m-stat--credits"><small>Credits</small><strong id="m-credits">—</strong></span>
        <span class="m-stat m-stat--link"><small>Link · tick</small><strong id="m-link">connecting…</strong></span>
        <span class="m-stat"><small>Contacts</small><strong id="m-contacts">—</strong></span>
        <span class="m-status__chevron" aria-hidden="true">⌄</span>
      </button>
      <div id="m-status-more" class="m-status-more" hidden>
        <span class="m-stat"><small>Corporation</small><strong id="m-corp">—</strong></span>
        <span class="m-stat"><small>ID</small><strong id="m-id">—</strong></span>
        <span class="m-stat"><small>Sim time</small><strong id="m-time">—</strong></span>
        <span class="m-stat"><small>Corps in view</small><strong id="m-corps">—</strong></span>
        <span class="m-stat"><small>Equity</small><strong id="m-equity">—</strong></span>
      </div>
    </header>

    <nav id="m-tabs" class="m-tabs" aria-label="Destinations" hidden>
      <button type="button" data-destination="market"><span aria-hidden="true">◇</span><small>Market</small></button>
      <button type="button" data-destination="fleets"><span aria-hidden="true">△</span><small>Fleets</small></button>
      <button type="button" data-destination="research"><span aria-hidden="true">⌬</span><small>Research</small></button>
      <button type="button" data-destination="officers"><span aria-hidden="true">★</span><small>Officers</small></button>
      <button type="button" data-destination="operations"><span aria-hidden="true">◎</span><small>Ops</small></button>
      <button type="button" data-destination="syndicate"><span aria-hidden="true">⬡</span><small>Syndicate</small></button>
      <button type="button" data-destination="faction"><span aria-hidden="true">⚖</span><small>Faction</small></button>
      <button type="button" data-destination="log"><span aria-hidden="true">⌖</span><small>Log</small></button>
    </nav>

    <div id="m-join" class="m-join">
      <form id="m-join-form" class="m-join__card">
        <div class="m-join__eyebrow">Corporate command uplink</div>
        <h1>Stellar Syndicates</h1>
        <p>Charter a corporation or reconnect with its existing name.</p>
        <label for="m-name">Corporation name</label>
        <input id="m-name" type="text" placeholder="e.g. Meridian Freight" autocomplete="off" />
        <button id="m-join-button" type="submit">Charter &amp; connect</button>
        <div id="m-join-error" class="m-join__error" aria-live="polite"></div>
      </form>
    </div>
  </div>
`;

export function mountMobileMarkup(root: HTMLElement): void {
  root.innerHTML = mobileMarkup;
}
