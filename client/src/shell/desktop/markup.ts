export const desktopMarkup = String.raw`
    <div id="hud">
      <div class="item"><span class="k">Corp</span><span class="v accent" id="hud-name">—</span></div>
      <div class="item"><span class="k">ID</span><span class="v" id="hud-id">—</span></div>
      <div class="item"><span class="k">Tick</span><span class="v" id="hud-tick">—</span></div>
      <div class="item"><span class="k">Sim&nbsp;t</span><span class="v" id="hud-time">—</span></div>
      <div class="item"><span class="k">Corps&nbsp;in&nbsp;view</span><span class="v" id="hud-online">—</span></div>
      <div class="item"><span class="k">Contacts</span><span class="v" id="hud-ships">—</span></div>
      <div class="item"><span class="k">Credits</span><span class="v accent" id="hud-credits">—</span></div>
      <div class="item"><span class="k">Equity</span><span class="v" id="hud-equity">—</span></div>
      <div class="spacer"></div>
      <nav class="hud-nav" aria-label="Hub destinations">
        <button id="nav-market" class="hud-btn" type="button" title="Hub Exchange (M)"><img class="cicon" src="/art/ui_icons/svg/concept-market-exchange.svg" width="14" height="14" alt="" />Market</button>
        <button id="nav-fleets" class="hud-btn" type="button" title="All fleets — docked and under way (V)"><img class="cicon" src="/art/ui_icons/svg/concept-fleet.svg" width="14" height="14" alt="" />Fleets</button>
        <button id="nav-research" class="hud-btn" type="button" title="Research — programme boards (R)">🔬 Research</button>
        <button id="nav-officers" class="hud-btn" type="button" title="Officers — Captain roster (P)">★ Officers</button>
        <button id="nav-operations" class="hud-btn" type="button" title="Operations — contracts and strategic objectives (U)">◎ Operations</button>
        <button id="nav-syndicate" class="hud-btn" type="button" title="Syndicate — alliances (Y)">🤝 Syndicate</button>
        <button id="nav-faction" class="hud-btn" type="button" title="Faction — your charter with the Terran Charter Authority (C)">⚖ Faction</button>
        <button id="nav-log" class="hud-btn" type="button" title="Check-in log (L)">⌖ Log</button>
      </nav>
      <div class="item"><span class="k">Link</span><span class="v" id="hud-link">connecting…</span></div>
    </div>

    <div id="reports-log"></div>

    <!-- System View breadcrumb (semantic-zoom LOD) — GALAXY › SYSTEM, clickable to
         return. Shown only inside the System View; the map is otherwise untouched. -->
    <div id="breadcrumb">
      <button id="bc-galaxy" class="bc-link" type="button" title="Back to galaxy (Esc)">GALAXY</button>
      <span class="bc-sep">›</span>
      <span id="bc-system" class="bc-cur"></span>
      <button id="bc-back" class="bc-back" type="button" title="Back to galaxy (Esc)">← Back</button>
    </div>

    <!-- Planet details — a read-only card for a clicked planet/moon in the System
         View. Presentation only: name, kind, the SYSTEM deposit visually associated
         here, and flavor. NOT a deeper camera level and NOT a gameplay action. -->
    <div id="planet-panel"></div>
    <div id="build-panel" class="build-shell"></div>
    <div id="build-ship-panel" class="build-shell"></div>

    <!-- §battle-aftermath: the battle-results panel (planet-panel idiom, ember-
         striped). Opened by an aftermath map marker or a battle log entry;
         strictly owner-only content (the View only carries YOUR reports). -->
    <div id="battle-panel"></div>

    <!-- §battle-records: the BATTLE VIEWER — the light-cone replay overlay. A
         centered house-width panel (above the market) built by main.ts; opened
         from a battle panel / inbox "View battle" affordance. -->
    <div id="battle-viewer"></div>

    <!-- §ground G3: the GROUND THEATER — a landing replayed. Same light-cone
         discipline as the battle viewer: everything shown has already arrived. -->
    <div id="ground-viewer"></div>

    <!-- Wormhole Hub detail — portrait + blurb + Open Market shortcut when the
         hub landmark is clicked on the galaxy map. Public geography; info only. -->
    <div id="hub-panel"></div>

    <!-- §syndicates Part 1: the alliance panel — create / invite / roster / accept
         / leave / dissolve. Opened from the navbar (🤝) or \`Y\`. Owner-only content
         (your own roster + pending invites; never a rival's private roster). -->
    <div id="syndicate-panel"></div>
    <div id="operations-panel"></div>

    <!-- §TCA: the FACTION panel — your charter standing with the Terran Charter
         Authority: the band ladder, what the band is costing you, and the
         reinstatement desk. Opened from the navbar (⚖) or \`C\`. Owner-only. -->
    <div id="faction-panel"></div>
    <div id="research-panel"></div>

    <!-- Unified right-rail workspace (Stellar-Charters-inspired master→detail):
         System / Market / Logistics / Doctrine as a tab stack. One column beside
         the map, one tab at a time, closes cleanly — the map stays uncluttered. -->
    <div id="rail">
      <div class="rail__tabs">
        <div class="seg" id="rail-tabs">
          <button data-tab="system">System</button>
          <button data-tab="fleets">Fleets</button>
          <button data-tab="logistics">Logistics</button>
          <button data-tab="doctrine">Doctrine</button>
          <button data-tab="officers">Officers</button>
          <button data-tab="rankings">Rankings</button>
        </div>
        <button class="rail__close" id="rail-close" type="button" title="Close (Esc)" aria-label="Close workspace">✕</button>
      </div>
      <div class="rail__body">
        <!-- SYSTEM tab — rich master→detail star-system view, built by JS. -->
        <div id="tab-system" class="tab"></div>

        <!-- FLEETS — every owner fleet, including hulls hidden while docked. -->
        <div id="tab-fleets" class="tab"></div>

        <!-- LOGISTICS tab — standing orders (§15), automation that runs offline. -->
        <div id="standing" class="tab">
          <div class="panel-title"><div><div class="eyebrow">automation · §15</div><h2>Standing Orders</h2></div></div>
          <div id="standing-list" class="dim">No standing orders yet.</div>
          <div class="so-form">
            <div class="so-row"><label>From</label><select id="so-source"></select></div>
            <div class="so-row"><label>Ship</label><select id="so-commodity"></select></div>
            <div class="so-row"><label>When</label>
              <select id="so-trigger">
                <option value="above_threshold">stockpile ≥</option>
                <option value="percent_surplus">% of surplus over</option>
                <option value="maintain_at_dest">keep dest ≥</option>
              </select>
              <input id="so-amount" type="number" step="1" min="0" value="10" title="threshold / percent / target" />
            </div>
            <div class="so-row" id="so-floor-row"><label>%→keep</label><input id="so-floor" type="number" step="1" min="0" value="50" title="percent (1-100) and floor to leave behind" /></div>
            <div class="so-row"><label>To</label><select id="so-dest"></select></div>
            <div class="so-row" id="so-sell-row"><label>On arrival</label><label class="so-check" title="Market Hub deliveries only: SELL the lot on the quantity-aware market curve at arrival, or leave it in your Market Warehouse to trade later."><input id="so-sell" type="checkbox" checked /> sell at the hub</label></div>
            <button id="so-add">Add standing order</button>
          </div>
          <div class="mhint dim">Rules run on the server — online or off. Each needs a real idle cargo fleet at its source and commands that hull when it fires; at most one run per rule. You set policy; geography + raids do the rest.</div>
        </div>

        <!-- DOCTRINE tab — fleet doctrine (§16), constrained policy, runs offline. -->
        <div id="doctrine" class="tab">
          <div class="panel-title"><div><div class="eyebrow">policy · §16</div><h2>Fleet Doctrine</h2></div></div>
          <div class="fd-row"><label>Engage</label><select id="fd-engage"></select></div>
          <div class="fd-row"><label>Retreat</label><select id="fd-retreat"></select></div>
          <div class="fd-row"><label>Escort</label><select id="fd-escort"></select></div>
          <div class="fd-row"><label>Lost&nbsp;supply</label><select id="fd-dest"></select></div>
          <div class="mhint dim">Standing policy your pickets follow autonomously — online or off. Defaults match today's behaviour. Pickets sense only what's in range; the ships they command stay raidable &amp; light-revealed.</div>
        </div>

        <!-- OFFICERS — Academy recruitment, reserve duty and fleet assignment. -->
        <div id="tab-officers" class="tab"></div>

        <!-- RANKINGS tab — the published leaderboard (§rankings), snapshot on the
             ledger close. Public by design: the exchange's quarterly ledger. -->
        <div id="tab-rankings" class="tab">
          <div class="panel-title"><div><div class="eyebrow">published ledger · §rankings</div><h2>Rankings</h2></div></div>
          <div id="rankings-body" class="dim">No ledger published yet.</div>
          <div class="mhint dim">The exchange publishes this ledger each close (every 60 s) — the same table for every corporation. Click a column to sort; your corp is highlighted. Category leaders wear a title.</div>
        </div>
      </div>
    </div>

    <!-- SHIP DETAILS — a master→detail card for a SELECTED ship, fog-aware (own vs
         rival). Shares the right-dock slot with the rail (mutually exclusive: a ship
         and a system are never both selected). Filled by JS; closes on deselect. -->
    <div id="ship-panel"></div>

    <!-- §management-home: the SYSTEM VIEW's management column — where an OWNED
         system is RUN (the city-screen pattern). Hosts the build/develop menu,
         stockpile + cap, production and queue — every command it issues is the
         SAME system-level command the galaxy rail used to send (buildings consume
         SYSTEM slots; no per-planet gameplay). Shown only inside the System View
         of a system you own; rivals' views stay pure scenery. The static shell
         here never gets replaced (only #svm-body's innerHTML), so its one
         delegated click listener survives every re-render. -->
    <div id="sysview-manage">
      <div class="svm-head">
        <div><div class="eyebrow" id="svm-eyebrow">system management</div><h2 id="svm-title" style="margin:0;font-size:16px">—</h2></div>
        <div style="display:flex;align-items:center;gap:10px">
          <span class="svm-slots" id="svm-slots" title="development slots used / total — each Mining Complex/Orbital Warehouse/Shipyard/… tier uses one; ships never do">—</span>
          <button class="svm-close" id="svm-close" type="button" title="Back to galaxy (Esc)" aria-label="Close management">✕</button>
        </div>
      </div>
      <div class="svm-body" id="svm-body"></div>
    </div>

    <!-- HUB EXCHANGE — a galaxy-wide institution, NOT tied to any selected system,
         so it opens from the TOP NAVBAR (and M), independent of the right rail. -->
    <div id="market">
      <div class="panel-title"><div><div class="eyebrow">the shared commons · light-delayed</div><h2>Wormhole Hub</h2></div><div class="panel-title__right"><span id="market-fresh" class="badge badge--neutral"></span><button class="rail__close" id="market-close" type="button" title="Close (Esc)" aria-label="Close market">✕</button></div></div>
      <div id="market-wallet" class="stat-strip"></div>
      <div class="market-tabs" id="market-tabs" role="tablist" aria-label="Market sections">
        <button id="market-tab-exchange" data-mtab="exchange" class="is-active" type="button" role="tab" aria-controls="market-pane-exchange" aria-selected="true">Exchange</button>
        <button id="market-tab-warehouse" data-mtab="warehouse" type="button" role="tab" aria-controls="market-pane-warehouse" aria-selected="false" tabindex="-1"><img class="icon icon--sm" src="/art/ui_icons/panel/concept-warehouse.png" alt="" />Warehouse</button>
        <button id="market-tab-specialists" data-mtab="specialists" type="button" role="tab" aria-controls="market-pane-specialists" aria-selected="false" tabindex="-1">Specialists</button>
        <button id="market-tab-modules" data-mtab="modules" type="button" role="tab" aria-controls="market-pane-modules" aria-selected="false" tabindex="-1"><img class="icon icon--sm" src="/art/ui_icons/panel/module-mass-driver.png" alt="" />Modules</button>
      </div>
      <div id="market-pane-exchange" class="mkt-exchange" role="tabpanel" aria-labelledby="market-tab-exchange">
        <div class="ux-stack">
          <section class="ux-section">
            <div class="ux-section__head"><img class="icon icon--sm" src="/art/ui_icons/panel/concept-stockpile.png" alt="" /><span>Market prices</span></div>
            <div class="board__head"><span></span><span>commodity</span><span>history</span><span>price · trend</span><span title="Units of this good in your Market Warehouse — the only pool Buy/Sell settle against. The other place your goods can sit is a system's stockpile.">STORED</span></div>
            <div id="market-board"></div>
          </section>
        </div>
        <div class="ux-stack">
          <section class="ux-section ux-section--primary">
            <div class="ux-section__head"><img class="icon icon--sm" src="/art/ui_icons/panel/concept-warehouse.png" alt="" /><span>Trade ticket</span></div>
            <div class="composer composer--flush">
            <div class="composer__row"><label>Order</label>
              <div class="seg" id="mk-side"><button data-side="buy" class="is-active">Buy</button><button data-side="sell">Sell</button></div>
              <span class="composer__sel" id="mk-sel">—</span>
            </div>
            <div class="composer__row"><label>Qty</label><input type="number" id="mk-qty" min="1" value="50" />
              <label class="lim"><input type="checkbox" id="mk-limit-on" /> limit @</label><input type="number" id="mk-limit" step="0.1" placeholder="mkt" disabled /></div>
            <div class="composer__preview" id="mk-preview"></div>
            <button class="act act--primary" id="mk-submit">Buy</button>
            </div>
          </section>
          <section class="ux-section">
            <div class="ux-section__head"><img class="icon icon--sm" src="/art/ui_icons/panel/status-construction-queue.png" alt="" /><span>Open Orders</span></div>
            <div class="mkt-orders" id="market-orders"></div>
          </section>
          <section class="ux-section">
            <div class="ux-section__head"><img class="icon icon--sm" src="/art/ui_icons/panel/status-construction-queue.png" alt="" /><span>Incoming Orders</span></div>
            <div class="mkt-orders" id="market-incoming-orders"></div>
          </section>
          <section class="ux-section">
            <div class="ux-section__head"><img class="icon icon--sm" src="/art/ui_icons/panel/status-construction-queue.png" alt="" /><span>Recent Orders</span></div>
            <div class="mkt-orders" id="market-recent-orders"></div>
          </section>
        </div>
      </div>
      <!-- §TCA → §market-ux: the WAREHOUSE tab — your stock at the hub and the ONE
           place goods cross between it and your systems. Both channels live here,
           chosen by the CARRIER toggle, because they are the same decision made two
           ways: the Authority's scheduled hull (a fee, a timetable, someone else's
           risk to fly) or one of your own freighters (free, immediate, yours to lose).
           Your charter STANDING, which prices the Authority's half, is in Faction. -->
      <div id="market-pane-warehouse" class="ux-stack" role="tabpanel" aria-labelledby="market-tab-warehouse" hidden>
        <!-- §TCA: the WAREHOUSE — the only stock the Exchange trades against. Rows
             are clickable: picking one loads it into the shipping composer below. -->
        <section class="ux-section">
          <div class="ux-section__head"><img class="icon icon--sm" src="/art/ui_icons/panel/concept-warehouse.png" alt="" /><span>Warehouse inventory</span><small>click a good to ship it</small></div>
          <div id="wh-table" class="mkt-orders"></div>
        </section>
        <section class="ux-section">
          <div class="ux-section__head"><img class="icon icon--sm" src="/art/ui_icons/panel/concept-manifest.png" alt="" /><span>Docked fleets</span></div>
          <div id="wh-berths"></div>
        </section>

        <section class="ux-section ux-section--primary">
          <div class="ux-section__head"><img class="icon icon--sm" src="/art/ui_icons/panel/concept-freight-route.png" alt="" /><span>Book freight</span></div>
          <div class="composer composer--flush">
          <div class="composer__row"><label><img class="icon icon--sm" src="/art/ui_icons/panel/concept-authority-freighter.png" alt="" />Carrier</label>
            <div class="seg" id="fr-carrier">
              <button data-carrier="tca" class="is-active" title="The Terran Charter Authority's scheduled common carrier. Costs a fee (charged now and destroyed), rides a fixed timetable, and can carry goods BOTH ways. The freighter is a real, raidable hull — but it isn't yours to lose.">Authority freight</button>
            </div>
            <span class="dim" title="To haul personally, select a real cargo fleet at this hub, load it, and issue its route from the fleet panel.">Owned hulls are managed from their fleet panel.</span>
          </div>
          <div class="composer__row"><label><img class="icon icon--sm" src="/art/ui_icons/panel/concept-freight-route.png" alt="" />Direction</label>
            <div class="seg" id="fr-dir"><button data-dir="outbound" class="is-active">Warehouse → system</button><button data-dir="inbound">System → warehouse</button></div>
          </div>
          <div class="composer__row"><label>System</label><select id="fr-system"></select></div>
          <div class="composer__row"><label><img class="icon icon--sm" src="/art/ui_icons/panel/concept-manifest.png" alt="" />Goods</label><select id="fr-commodity"></select>
            <label>Qty</label><input type="number" id="fr-qty" min="1" value="100" />
            <button class="act" id="fr-add" title="Add or update this commodity in the mixed freight manifest."><img class="icon icon--sm" src="/art/ui_icons/panel/concept-manifest.png" alt="" />Add cargo</button></div>
          <div id="fr-manifest" class="mkt-orders"></div>
          <div class="composer__row" id="fr-sell-row"><label>On arrival</label><label class="lim" title="Sell the lot at the Global Market the moment it lands at the Market Hub, on that tick's quantity-aware curve."><input type="checkbox" id="fr-sell" /> sell on arrival</label></div>
          <div class="composer__preview" id="fr-preview"></div>
          <button class="act act--primary" id="fr-submit"><img class="icon icon--sm" src="/art/ui_icons/panel/concept-authority-freighter.png" alt="" />Book freight</button>
          <div class="mhint" id="fr-feedback"></div>
          </div>
        </section>
        <section class="ux-section">
          <div class="ux-section__head"><img class="icon icon--sm" src="/art/ui_icons/panel/concept-authority-freighter.png" alt="" /><span>Shipments in hand</span></div>
          <div id="fr-queue" class="mkt-orders"></div>
        </section>
      </div>
      <div id="market-pane-specialists" class="ux-stack" role="tabpanel" aria-labelledby="market-tab-specialists" hidden>
        <section class="ux-section"><div class="ux-section__head"><span>Available specialists</span></div>
          <div id="sp-rows"></div><div class="mhint" id="sp-feedback"></div>
        </section>
      </div>
      <div id="market-pane-modules" class="ux-stack" role="tabpanel" aria-labelledby="market-tab-modules" hidden>
        <section class="ux-section"><div class="ux-section__head"><img class="icon icon--sm" src="/art/ui_icons/panel/module-mass-driver.png" alt="" /><span>Combat modules</span></div>
          <div id="mod-rows"></div><div class="mhint" id="mod-feedback"></div>
        </section>
      </div>
    </div>

    <!-- Check-in timeline + attention (§16, Layer 3) — the welcome-back digest. -->
    <div id="checkin">
      <div class="mhead"><b>CHECK-IN</b> <span id="checkin-toggle" class="dim" title="Press L">✕</span></div>
      <div class="ci-sub" id="checkin-att-head">Decision inbox</div>
      <div id="checkin-attention"><span class="dim">Loading…</span></div>
      <div class="ci-sub" id="checkin-log-head" style="margin-top:16px">Log</div>
      <div id="checkin-timeline"><span class="dim">Loading…</span></div>
      <div class="mhint dim">What became observable while you were away (own economy is instant; distant battles &amp; rival claims arrive light-delayed). Attention items are nudges from your own view. Press <b>L</b> to toggle.</div>
    </div>

    <aside id="founding-guide" aria-live="polite"></aside>
    <div id="readout" aria-live="polite"></div>
    <div id="intent-bar" aria-live="polite"></div>

    <div id="zoom-controls">
      <button id="zoom-in" type="button" title="Zoom in (scroll up · +)" aria-label="Zoom in">+</button>
      <output id="zoom-level" title="Galaxy magnification relative to the fit-to-map view" aria-label="Map zoom level">1.0×</output>
      <button id="zoom-reset" type="button" title="Fit galaxy (reset view)" aria-label="Fit galaxy">⊡</button>
      <button id="zoom-out" type="button" title="Zoom out (scroll down · −)" aria-label="Zoom out">−</button>
    </div>

    <div id="legend">
      <button id="legend-toggle" type="button" aria-expanded="true" aria-controls="legend-body">
        <span id="legend-toggle-label">Hide map help</span><span class="legend-toggle__chev" aria-hidden="true">▾</span>
      </button>
      <div id="legend-body">
        <div class="row"><span class="dot" style="background:#4fc3ff"></span> your ships / systems — delayed, crisp only near HQ</div>
        <div class="row"><span class="dot" style="background:#ff7a6b"></span> rivals — claims &amp; ghosts arrive light-delayed</div>
        <div class="row"><img class="cicon" src="/art/ui_icons/svg/concept-command-center-hq.svg" width="13" height="13" alt="" /> command center · <img class="cicon" src="/art/ui_icons/svg/concept-sensor-range.svg" width="13" height="13" alt="" /> sensor range</div>
        <div class="row"><span class="dim">colonize (send a colony ship) → produce → ship to hub (raidable) → sell</span></div>
        <div class="row"><span class="dim">click a system = open it · raider + click rival = raid · R = recall · Δt = staleness</span></div>
        <div class="row"><span class="dim">scroll = zoom · drag = pan · ⊡ = fit · arrows/+/− also work</span></div>
        <div class="row"><span class="dim">rail: S = system · O = logistics · F = doctrine · Esc = close</span></div>
        <div class="row"><span class="dim">top bar: M = market · L = check-in log</span></div>
      </div>
    </div>

    <div id="join">
      <picture class="join-art" aria-hidden="true">
        <source type="image/webp" srcset="/art/derived/lore/corporate_command_center-768.webp 768w, /art/derived/lore/corporate_command_center-1280.webp 1280w" sizes="100vw">
        <img src="/art/lore_illustrations/corporate_command_center.png" alt="" decoding="async" fetchpriority="high">
      </picture>
      <div class="card">
        <h1>Stellar Syndicates</h1>
        <p>Charter a corporation and command from your home anchor. Enter a name to join the galaxy — reconnecting with the same name resumes your corporation.</p>
        <label for="name">Corporation name</label>
        <input id="name" type="text" placeholder="e.g. Meridian Freight" autocomplete="off" />
        <button id="join-btn">Charter &amp; connect</button>
        <div class="err" id="join-err"></div>
      </div>
    </div>
`;

export function mountDesktopMarkup(root: HTMLElement): void {
  root.innerHTML = desktopMarkup;
}
