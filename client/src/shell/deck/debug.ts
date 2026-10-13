import type { BattleRecordView, BattleView, BodyView, CountClass, GhostView, GroundRecordView, KeyframeView, ShipKind, SystemStateView } from "../../protocol";
import type { CoreContext } from "../types";
import { liveSimTime } from "../../state";
import { deriveSystemPicture, type SystemPicture } from "../../systemtraffic";

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
  const stopLive = (): void => {
    if (liveTimer !== null) window.clearInterval(liveTimer);
    liveTimer = null;
  };
  /** Open a real engine fight (`mixed`, `swarm`, `torpedo`, `titan`: see
   * /dev/battles/index.json), or the old hand-scripted demo with
   * `theaterDemo("scripted", titanDown?, big?)`. Engine fights are recorded on
   * the current battle rules by the server's `battle_fixtures` test. */
  const theaterDemo = async (fight = "mixed", titanDown = false, big = false): Promise<BattleRecordView> => {
    stopLive();
    const record = fight === "scripted" ? buildTheaterDemo(titanDown, big) : await engineFight(fight);
    install(record);
    openBattle(record.id);
    return record;
  };
  /** The same fight arriving round by round, as a live battle's light does. */
  const theaterDemoLive = async (intervalMs = 1800, fight = "mixed"): Promise<BattleRecordView> => {
    stopLive();
    const full = fight === "scripted" ? buildTheaterDemo(false, false) : await engineFight(fight);
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
    stopLive();
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

  /** A synthetic System View picture for the star in view: berths at every
   * kind of facility, near-zone traffic, an inbound pirate, an own freighter
   * and a rival just leaving, a battle in orbit, a wreck field and a siege with
   * a landing. Rendering only — nothing is sent,
   * and served Views never touch it. `systemDemo(false)` restores truth. */
  const systemDemo = (on = true): SystemPicture | null => {
    const mode = ctx.renderer.viewMode;
    if (!on || mode.type !== "system") {
      ctx.renderer.setSystemDemo(null);
      return null;
    }
    const built = buildSystemDemo(ctx, mode.systemId);
    if (!built) return null;
    ctx.renderer.setSystemDemo(built);
    return built.picture;
  };

  const installed = {
    ...previous,
    state: ctx.state,
    renderer: ctx.renderer,
    net: ctx.net,
    theaterDemo,
    theaterDemoLive,
    theaterDemoSemantic,
    systemDemo,
    openBattleViewer: openBattle,
    openGroundViewer: openGround,
  };
  target.__ss = installed;
  return () => {
    if (liveTimer !== null) window.clearInterval(liveTimer);
    if (target.__ss === installed) target.__ss = previous;
  };
}

/** A real engine fight from client/public/dev/battles (the server's
 * `battle_fixtures` test writes them), under a demo id. */
async function engineFight(name: string): Promise<BattleRecordView> {
  const response = await fetch(`/dev/battles/${encodeURIComponent(name)}.json`);
  if (!response.ok) {
    const index = await fetch("/dev/battles/index.json").then((r) => (r.ok ? r.json() : []), () => []) as { name: string }[];
    throw new Error(`No engine fight "${name}". Try: ${index.map((entry) => entry.name).join(", ") || "mixed"}, or "scripted".`);
  }
  const record = await response.json() as BattleRecordView;
  return { ...record, id: `engine-${name}` };
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

function buildSystemDemo(ctx: CoreContext, systemId: string): { picture: SystemPicture; report: SystemStateView } | null {
  const state = ctx.state;
  const system = state.galaxy?.systems.find((entry) => entry.id === systemId);
  const real = state.systems.find((entry) => entry.id === systemId);
  if (!system || !real || !state.galaxy || !real.bodies.length) return null;
  const now = liveSimTime();
  const me = state.playerId ?? "1";
  const rival = "demo-rival";
  const pirate = state.galaxy.pirate_id ?? "demo-pirate";
  const bodies: BodyView[] = real.bodies.map((body) => ({ ...body, structures: { ...body.structures } }));
  const main = bodies.find((body) => body.population > 0) ?? bodies.find((body) => body.habitable) ?? bodies[0];
  main.population = Math.max(main.population, 1.4);
  Object.assign(main.structures, { naval_drydock: 3, capital_slipway: 1, defense_platform: 3, orbital_warehouse: 2, sensor_array: 2, shipyard: 2, garrison: 1 });
  const moon = bodies.find((body) => body.parent !== null) ?? bodies.find((body) => body !== main) ?? main;
  moon.structures.mining_complex = Math.max(1, moon.structures.mining_complex ?? 0);
  const report: SystemStateView = {
    ...real,
    owner: me,
    bodies,
    builds: [{ key: "cruiser", body_id: main.id, yard: "naval_drydock", complete_time: null }],
    converters: [{ body_id: moon.id, structure: "smelter", title: "Smelter", tier: 1, status: "needs_crew" }],
    defense_tier: 3,
    food_state: "well_supplied",
    blockade: { by: rival, by_name: "Vesper Combine", since: now - 900, by_me: false, held_since: now - 700,
      siege_since: now - 0.43 * (state.galaxy.siege_secs || 600), siege_secs: state.galaxy.siege_secs || 600, capturable: true },
    ground: { garrison_tier: 2, garrison_fed: true, suppression: 0.37, marines_needed: 40 },
  };
  const star = system.pos;
  const at = (dx: number, dy: number) => ({ x: star.x + dx, y: star.y + dy });
  const toward = (pos: { x: number; y: number }, speed: number) => {
    const d = Math.hypot(star.x - pos.x, star.y - pos.y) || 1;
    return { x: (star.x - pos.x) / d * speed, y: (star.y - pos.y) / d * speed };
  };
  const ghost = (id: string, kind: ShipKind, opts: Partial<GhostView> & { n?: number } = {}): GhostView => {
    const n = opts.n ?? 1;
    const own = opts.own ?? false;
    return {
      id, kind, owner: own ? me : rival, pos: star, vel: { x: 0, y: 0 }, age: 0, own, jumped: false,
      route: null, passengers: {}, cargo: null, count_class: n === 1 ? "one" : n <= 3 ? "two_to_three" : "four_to_seven",
      composition: own || opts.composition !== undefined ? [{ kind, count: n }] : null,
      signature: null, posture: null, ...opts,
    } as GhostView;
  };
  const berth = `E${systemId}`;
  // An own freighter that left 45 s ago for the nearest other star.
  const other = state.galaxy.systems.filter((entry) => entry.id !== systemId)
    .sort((a, b) => Math.hypot(a.pos.x - star.x, a.pos.y - star.y) - Math.hypot(b.pos.x - star.x, b.pos.y - star.y))[0];
  const away = other ? Math.atan2(other.pos.y - star.y, other.pos.x - star.x) : 2.76;
  const leaving = at(Math.cos(away) * 2750, Math.sin(away) * 2750);
  const ghosts: GhostView[] = [
    ghost("demo-1", "raider", { own: true, docked: berth }),
    ghost("demo-2", "scout", { own: true, docked: berth }),
    ghost("demo-3", "tiny_freighter", { own: true, docked: berth }),
    ghost("demo-4", "cruiser", { own: true, docked: berth, n: 3 }),
    ghost("demo-5", "convoy", { own: true, docked: berth, n: 2 }),
    ghost("demo-6", "freighter", { tca: true, owner: "tca", docked: berth }),
    ghost("demo-7", "raider", { pos: at(900, -480), vel: toward(at(900, -480), 40), age: 4, n: 3, composition: [{ kind: "raider", count: 3 }] }),
    ghost("demo-8", "corvette", { own: true, pos: at(-700, -520), vel: { x: 20, y: -30 }, age: 2 }),
    ghost("demo-9", "raider", { pirate: true, owner: pirate, pos: at(32000, 21000), vel: toward(at(32000, 21000), 52), age: 6, n: 4 }),
    ghost("demo-10", "destroyer", { pos: at(140, -60), age: 3, n: 2, composition: [{ kind: "destroyer", count: 2 }] }),
    ghost("demo-11", "transport", { pos: at(-120, 90), age: 3 }),
    ghost("demo-12", "corvette", { own: true, pos: at(700, 520), age: 1, n: 2 }),
    ghost("demo-13", "raider", { pirate: true, owner: pirate, pos: at(760, 560), age: 1, n: 3 }),
    ghost("demo-14", "large_freighter", { own: true, pos: leaving, vel: { x: Math.cos(away) * 32, y: Math.sin(away) * 32 },
      speed: 32, path: other ? [{ pos: other.pos }] : null, age: 2 }),
    ghost("demo-15", "raider", { pos: at(-2400, -2600), vel: { x: -40, y: -44 }, age: 3, n: 2 }),
  ];
  const battle: BattleView = { id: "demo-system-battle", pos: at(730, 540), age: 1, started_at: now - 40, own: true, participants: ["demo-12", "demo-13"] };
  const count = (kind: ShipKind, exact: number) => ({ kind, exact, class: "one" as CountClass });
  const record = (id: string, pos: { x: number; y: number }, outcome: BattleRecordView["outcome"]): BattleRecordView => ({
    id, pos, system: systemId, started_at: now - 200, raid: false, fidelity: "participant", own_side: 1,
    sides: [
      { corp: pirate, posture: null, platform_tiers: 0, initial: [count("raider", 3)] },
      { corp: me, posture: "engage_any", platform_tiers: 3, initial: [count("corvette", 2)] },
    ],
    rounds: [{ tick: 1, counts: [[count("raider", 3)], [count("corvette", 2)]], kills: [[count("raider", 1)], []], dealt: [20, 30], notes: [] }],
    light_frontier_tick: 1,
    outcome,
  });
  const landing = {
    id: "demo-landing", system: systemId, pos: star, started_at: now - 60, fidelity: "participant", attacker: rival, defender: me,
    attacking: false, marines_landed: 60, defenders_initial: 40, garrison_tiers: 2, suppression_at_drop: 0.37,
    rounds: [{ tick: 1, marines: 38, defenders: 21, marines_frac: 0.64, defenders_frac: 0.52, suppression: 0.37, marine_losses: 4, defender_losses: 3 }],
    light_frontier_tick: 1, outcome: null,
  } as GroundRecordView;
  const picture = deriveSystemPicture({
    system, dynamic: report, galaxy: state.galaxy, ghosts, battles: [battle],
    battleRecords: [record(battle.id, battle.pos, null), record("demo-wreck", at(-900, 380), "target_destroyed")],
    groundRecords: [landing], battleDismissed: new Set(), commandCenter: state.commandCenter, playerId: me,
    simTime: now, selectedShipId: "demo-4", selectedShipIds: new Set(["demo-4"]), selectedBattleId: null,
    places: [...state.galaxy.systems.map((entry) => ({ name: entry.name, pos: entry.pos })), { name: "Market Hub", pos: state.galaxy.hub }],
  });
  return { picture, report };
}
