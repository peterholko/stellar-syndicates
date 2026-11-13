// §migration-status (Nov 1 2026): the Population tab's housing and liner
// status line (core/derive/migration.ts), its wiring on both shells, and the
// hold slugs against the sim's enum.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const src = (file) => readFileSync(new URL(`../src/${file}`, import.meta.url), "utf8");
const migration = {};
vm.runInNewContext(ts.transpileModule(src("core/derive/migration.ts"), { compilerOptions: {
  module: ts.ModuleKind.CommonJS, target: ts.ScriptTarget.ES2022 } }).outputText, { exports: migration, require: () => ({}) });
const { housingText, migrationStatus, MIGRATION_HOLDS } = migration;

const now = 1_000;
const world = (m) => ({ population: 0.007, migration: { housing: 0.0125, liners: 0, ...m } });
const line = (m) => migrationStatus(world(m), now);

assert.equal(housingText(world({})), "7,000 / 12,500", "people here / housing");
assert.equal(housingText({ population: 0.007 }), null, "a rival's world: no status");
assert.equal(migrationStatus({}, now), null);

assert.equal(line({ liners: 1, lands_at: now + 540 }).text, "Liner lands in 9m");
assert.equal(line({ liners: 1, lands_at: now + 500 }).text, "Liner lands in 9m", "rounds up to the minute");
assert.equal(line({ liners: 1, lands_at: now + 20 }).text, "Liner landing");
assert.equal(line({ liners: 2, lands_at: now + 120, hold: "staffed" }).text, "2 liners · first lands in 2m", "a landing outranks a hold");
assert.equal(line({ liners: 1 }).text, "Liner lands", "no timetable yet");

assert.deepEqual({ ...line({ hold: "housing" }) }, { text: "Housing full", tip: "Build or upgrade a Habitat.", warn: true });
assert.equal(line({ hold: "staffed", next_at: now + 60 }).text, "Posts staffed", "a hold outranks the timer");
assert.equal(line({ hold: "crew_free" }).text, "Crew idle");
assert.equal(line({ hold: "food" }).warn, true);
assert.equal(line({ next_at: now + 240 }).text, "Next liner in 4m");
assert.equal(line({ next_at: now - 5 }).text, "Next liner due");
assert.equal(line({}).text, "Queued", "eligible, but another world goes first");

// Every hold the sim can serve has words, and nothing else does.
const rust = readFileSync(new URL("../../crates/sim/src/migration.rs", import.meta.url), "utf8");
const body = rust.match(/pub enum MigrationHold \{([\s\S]*?)\n\}/)[1];
const slugs = [...body.matchAll(/^\s{4}([A-Z]\w*),/gm)].map(([, name]) => name.replace(/(?<!^)([A-Z])/g, "_$1").toLowerCase());
assert.deepEqual([...Object.keys(MIGRATION_HOLDS)].sort(), slugs.sort());
assert.match(src("protocol.ts"), /export type MigrationHold = "closed" \| "unsettled" \| "blockaded" \| "food" \| "housing" \| "crew_free" \| "staffed";/);
assert.match(src("protocol.ts"), /migration\?: MigrationView \| null;/);

const deck = src("shell/deck/empire.ts"), mobile = src("shell/mobile/parity.ts");
assert.match(deck, /stat\("Housing", housing, body\.migration\?\.hold === "housing"/, "desktop: housing");
assert.match(deck, /stat\("Immigration", status\.text, status\.warn, status\.tip, true\)/, "desktop: the status line, full width");
assert.match(mobile, /<small>Housing<\/small><b>\$\{housing\}<\/b>/, "mobile: housing");
assert.match(mobile, /<small>Immigration<\/small><b>\$\{esc\(status\.text\)\}<\/b>/, "mobile: the status line");
console.log("PASS: housing and liner status (landing, holds, next liner, queued) on both shells; holds match the sim.");
