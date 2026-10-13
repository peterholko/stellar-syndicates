# Performance backlog

Client and server performance work that has been diagnosed but not yet done.
Each entry says what happens today, the change, how to measure it, and where
it came from. Earlier, completed rounds are in the July 2026 perf audit
(quick wins, Pixi pooling, incremental battle records).

## 1. Pause the hidden game work while a full-screen overlay is open

Status: backlog (Oct 5 2026). Found while investigating choppy interceptor
motion in the in-game battle viewer (the viewer's 60 fps cap and the live
playback buffer, shipped the same day, were the other two findings).

**What happens today.** The desktop shell has no frame cap, so `boot.ts`
`frame()` runs once per display frame, 60 or 120 times a second. Each pass
calls `DeckShell.onViewTick()` (`client/src/shell/deck/index.ts`). Despite
its name, it runs every frame, not once per server update. It calls:
- `map.tick()`, `strip.render()`, `renderChrome()`, `renderZoom()`;
- nine panel `render(route)` calls: empire, command, welcome, market, policy,
  fleet, roster, log and strategic;
- `theaters.onViewTick()` and `syncBattleSelection()`.

While a theater, the join screen or help is open, `framePolicy()` pauses the
galaxy map's own drawing (`renderGalaxy: false`), but this whole list keeps
running underneath. Most panels return at once unless they own the current
route. Every frame, though:
- **the strip** builds a fingerprint of the selected fleets, orders and raids;
- **the panel for the route left open** builds its own fingerprint;
- **the theater hook** (`theaters.onViewTick()` → `renderBattle()`) builds a
  JSON signature of the battle card, then calls `theaterAttach()` again. That
  schedules a promise callback, which re-binds the record, calls `matchMedia`
  twice, resets the ticker cap and rewrites the "scroll zoom · drag pan" hint
  text. The text write is a DOM mutation every frame, so the browser redoes
  style and layout work for the overlay every frame.

All of this shares the main thread with the battle viewer, so a long pass
delays the viewer's next frame and its motion turns uneven. The test viewer
(`client/scripts/battle-model-fixture.html`) runs none of it.

**The change.**
- **Skip the hidden work** while an opaque overlay is open: map tick, strip,
  chrome, zoom and panel renders. Server updates still apply to the state, so
  nothing is lost. On close, force one full render (`render(true)`,
  `renderChrome(true)`) so the panels catch up. The galaxy map already pauses
  and resumes this way.
- **Keep what the overlay needs:** the theater transport (its own rAF loop),
  the battle card (it changes about once a second or per round) and toasts.
- **Attach the theater only when needed:** when the card is rebuilt or the
  record changes, not every frame. (The hint text is already written only when
  it changes, since the follow camera, Oct 6 2026.)
- **Keep the side effects:** `strip.render()` also reconciles the fleet
  selection, `syncBattleSelection()` keeps the battle highlighted on the map,
  and the welcome panel tracks time away. Keep those running, or run them on
  close.

**How to measure.** Chrome DevTools, Performance tab: record about 5 s with
the battle viewer open, before and after. Compare the "Animation frame fired"
tasks from `boot.ts` `frame()` with the viewer's Pixi ticker frames, and the
spread of the viewer's frame intervals. Or measure on a scratch server with a
guest session (see the isolated playtest server recipe).

**Expected impact.** Not measured. Most calls exit early, so it may be small
for a small empire. It grows with the number of fleets, orders and raids, and
doubles on a 120 Hz screen.
