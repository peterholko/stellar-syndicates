import type { BattleRecordView, CountClass, KeyframeView, ShipKind } from "../../protocol";
import type { CoreContext } from "../types";

type DebugWindow = Window & typeof globalThis & { __ss?: Record<string, unknown> };

/** Keep the browser acceptance rig shell-agnostic. It exposes the same state,
 * renderer, net, and deterministic theater fixtures through the Deck-owned
 * overlays, including a semantic live-battle doorway. */
export function installDeckDebug(
  ctx: CoreContext,
  openBattle: (id: string) => void,
  openGround: (id: string) => void,
  enterBattle: (id: string) => void,
): () => void {
  const target = window as DebugWindow;
  const previous = target.__ss ?? {};
  let liveTimer: number | null = null;

  const install = (record: BattleRecordView): void => {
    ctx.state.battleRecords = ctx.state.battleRecords.filter((entry) => entry.id !== record.id).concat(record);
  };
  const theaterDemo = (titanDown = false, big = false): BattleRecordView => {
    if (liveTimer !== null) {
      window.clearInterval(liveTimer);
      liveTimer = null;
    }
    const record = buildTheaterDemo(titanDown, big);
    install(record);
    openBattle(record.id);
    return record;
  };
  const theaterDemoLive = (intervalMs = 1800): BattleRecordView => {
    if (liveTimer !== null) window.clearInterval(liveTimer);
    const full = buildTheaterDemo(false, false);
    let upto = Math.min(2, full.rounds.length);
    const update = (): void => {
      const record: BattleRecordView = {
        ...full,
        rounds: full.rounds.slice(0, upto),
        outcome: upto >= full.rounds.length ? full.outcome : null,
        light_frontier_tick: full.rounds[Math.max(0, upto - 1)]?.tick ?? 0,
      };
      install(record);
    };
    update();
    openBattle(full.id);
    liveTimer = window.setInterval(() => {
      upto = Math.min(full.rounds.length, upto + 1);
      update();
      if (upto >= full.rounds.length && liveTimer !== null) {
        window.clearInterval(liveTimer);
        liveTimer = null;
      }
    }, Math.max(100, intervalMs));
    return full;
  };
  const theaterDemoSemantic = (): BattleRecordView => {
    if (liveTimer !== null) window.clearInterval(liveTimer);
    liveTimer = null;
    const full = buildTheaterDemo(false, false);
    const record: BattleRecordView = {
      ...full,
      rounds: full.rounds.slice(0, 3),
      light_frontier_tick: full.rounds[2]?.tick ?? 0,
      outcome: null,
    };
    install(record);
    ctx.state.battles = ctx.state.battles.filter((entry) => entry.id !== record.id).concat({
      id: record.id,
      pos: record.pos,
      age: 0,
      started_at: record.started_at,
      own: true,
      participants: [],
    });
    enterBattle(record.id);
    return record;
  };

  const installed = {
    ...previous,
    state: ctx.state,
    renderer: ctx.renderer,
    net: ctx.net,
    theaterDemo,
    theaterDemoLive,
    theaterDemoSemantic,
    openBattleViewer: openBattle,
    openGroundViewer: openGround,
  };
  target.__ss = installed;
  return () => {
    if (liveTimer !== null) window.clearInterval(liveTimer);
    if (target.__ss === installed) target.__ss = previous;
  };
}

function buildTheaterDemo(titanDown: boolean, big: boolean): BattleRecordView {
  const mk = (side: number, kind: ShipKind, x: number, y: number, hp = 1, plat = false) => ({ side, kind, x, y, hp, plat });
  const rounds: BattleRecordView["rounds"] = [];
  const roundCount = 10;
  for (let i = 0; i < roundCount; i++) {
    const t = i / (roundCount - 1);
    const attackerX = -880 + 620 * t;
    const ships: KeyframeView["ships"] = [
      ...(titanDown && i >= 9 ? [] : [mk(0, "titan", attackerX - 60, 0, titanDown ? 1 - 0.9 * t : 1 - 0.25 * t)]),
      mk(0, "battleship", attackerX - 20, 120, 1 - 0.35 * t),
      ...[0, 1, 2, 3].map((k) => mk(0, "corvette", attackerX + 40, -160 + k * 90, 1 - 0.3 * t * ((k % 2) + 1) / 2)),
      ...[0, 1, 2, 3].map((k) => (i < 8 || k > 0 ? mk(0, "raider", attackerX + 90 + 30 * Math.sin(t * 6 + k), -220 + k * 140, 1 - 0.2 * t) : null)),
      ...[...Array(i < 5 ? 8 : i < 7 ? 6 : 4)].map((_, k) => mk(1, "raider", 320 + 25 * Math.cos(t * 5 + k), -260 + k * 76, 1 - 0.45 * t)),
      mk(1, "corvette", 250, -60, 1 - 0.3 * t),
      mk(1, "corvette", 250, 60, 1 - 0.3 * t),
      mk(1, "convoy", 540, 30, 1 - 0.5 * t),
      mk(1, "corvette", 600, 0, 1 - 0.6 * t, true),
      mk(1, "corvette", 600, 40, 1, true),
    ].filter((ship): ship is NonNullable<typeof ship> => ship !== null);
    if (big) {
      for (let k = 0; k < 46; k++) {
        ships.push(mk(0, k % 3 === 0 ? "raider" : "corvette", attackerX + 60 + (k % 8) * 34, -300 + Math.floor(k / 8) * 52, 1 - 0.3 * t));
      }
      ships.push(mk(0, "dreadnought", attackerX - 90, -80, 1 - 0.2 * t));
      for (let k = 0; k < 44; k++) {
        ships.push(mk(1, k % 4 === 0 ? "corvette" : "raider", 300 + (k % 8) * 30, -280 + Math.floor(k / 8) * 50, 1 - 0.4 * t));
      }
      ships.push(mk(1, "battleship", 560, -60, 1 - 0.35 * t), mk(1, "cruiser", 560, 90, 1 - 0.3 * t));
    }
    const torpedoes: KeyframeView["torpedoes"] = i >= 2 && i <= 8
      ? [{ side: 0, x: attackerX + 200 + 180 * ((i % 3) / 3), y: -20, n: Math.max(2, 10 - i) }]
      : [];
    const deaths: KeyframeView["deaths"] = [];
    if (i === 5) deaths.push({ step: 3, side: 1, kind: "raider", x: 340, y: -110 });
    if (i === 7) deaths.push({ step: 1, side: 1, kind: "raider", x: 355, y: 30 }, { step: 4, side: 1, kind: "raider", x: 310, y: 96 });
    if (i === 8) deaths.push({ step: 2, side: 0, kind: "raider", x: attackerX + 90, y: -220 });
    if (titanDown && i === 9) deaths.push({ step: 3, side: 0, kind: "titan", x: attackerX - 60, y: 0 });
    const count = (kind: ShipKind, exact: number) => ({ kind, exact, class: "one" as CountClass });
    rounds.push({
      tick: i * 15,
      counts: [
        [count("titan", titanDown && i >= 9 ? 0 : 1), count("battleship", 1), count("corvette", 4), count("raider", i < 8 ? 4 : 3)],
        [count("raider", i < 5 ? 8 : i < 7 ? 6 : 4), count("corvette", 2), count("convoy", 1)],
      ],
      kills: [
        [i === 8 ? count("raider", 1) : count("raider", 0)].filter((entry) => entry.exact),
        [i === 5 ? count("raider", 1) : i === 7 ? count("raider", 2) : count("raider", 0)].filter((entry) => entry.exact),
      ],
      dealt: [26 + i * 5, 18 + i * 3],
      notes: i === 6 ? [{ kind: "retreat_tripped", side: 1, comp: null }] : i === 9 ? [{ kind: "withdraw_ordered", side: 1, comp: null }] : [],
      frame: { ships, torpedoes, deaths },
    });
  }
  return {
    id: "demo-battle",
    pos: { x: 0, y: 0 },
    system: null,
    started_at: 0,
    raid: false,
    fidelity: "participant",
    own_side: 0,
    sides: [
      {
        corp: "1",
        posture: "engage_any",
        platform_tiers: 0,
        initial: rounds[0].counts[0],
        loadouts: [
          { kind: "raider", modules: ["torpedo_rack"], n: 4 },
          { kind: "corvette", modules: ["whipple_armor"], n: 2 },
        ],
        flagship_name: "Emberfall",
      },
      {
        corp: "2",
        posture: null,
        platform_tiers: 2,
        initial: rounds[0].counts[1],
        loadouts: [
          { kind: "corvette", modules: ["point_defense_screen"], n: 2 },
          { kind: "raider", modules: ["mass_driver"], n: 4 },
          { kind: "raider", modules: ["reflective_plating"], n: 3 },
        ],
        flagship_name: null,
      },
    ],
    rounds,
    light_frontier_tick: (roundCount - 1) * 15,
    outcome: "target_destroyed",
  };
}
