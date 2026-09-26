// Fully isolated command-stub tests. Never connects to PostgreSQL or the game.
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, readFileSync, realpathSync, writeFileSync, copyFileSync, chmodSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const script = fileURLToPath(new URL("./reset-database.sh", import.meta.url));
const fixture = realpathSync(mkdtempSync(join(tmpdir(), "stellar-reset-test-")));
try {
  for (const sub of ["scripts", ".devdb", "bin", "client"]) mkdirSync(join(fixture, sub));
  copyFileSync(script, join(fixture, "scripts/reset-database.sh"));
  const mock = `#!/usr/bin/env node
const fs=require('node:fs'), path=require('node:path');
const cmd=path.basename(process.argv[1]), args=process.argv.slice(2), env=process.env;
const log=(event)=>fs.appendFileSync(env.RESET_TEST_LOG, JSON.stringify({event,cmd,args})+'\\n');
const calls=fs.readFileSync(env.RESET_TEST_LOG,'utf8').trim().split('\\n').filter(Boolean).map(line=>JSON.parse(line));
const stopped=calls.some(call=>call.event==='stop');
if(cmd==='psql') {
  const sql=args.find(a=>a.startsWith('--command='))||'';
  if(sql.includes('TRUNCATE')) {log('delete');process.exit(env.RESET_TEST_FAIL==='delete'?1:0);}
  log('read');
  if(sql.includes('current_database')) console.log(env.RESET_TEST_FAIL==='database'?'unrelated_database':'stellar_syndicates');
  else if(sql.includes('data_directory')) console.log(env.RESET_TEST_FAIL==='cluster'||(env.RESET_TEST_FAIL==='cluster_late'&&stopped)?'/unrelated/postgres':path.join(env.RESET_TEST_ROOT,'.devdb'));
  else if(sql.includes('count(*)')) console.log('8');
  process.exit(0);
}
if(cmd==='lsof') {
  log('read');
  if(env.RESET_TEST_FAIL==='inspect') process.exit(2);
  if(args.includes('-Fn')) {console.log(env.RESET_TEST_SERVER==='1'?'n'+path.join(env.RESET_TEST_ROOT,'target/release/server'):'n/unrelated/server');process.exit(0);}
  if(env.RESET_TEST_SERVER==='1') {
    if(stopped&&env.RESET_TEST_FAIL==='replacement') {console.log('999998');process.exit(0);}
    if(stopped) process.exit(1);
    console.log(env.RESET_TEST_FAIL==='listener_change'&&calls.some(call=>call.event==='build')?'999998':'999999');
    process.exit(0);
  }
  if(env.RESET_TEST_FAIL==='listener') console.log('999999');
  process.exit(env.RESET_TEST_FAIL==='listener'?0:1);
}
if(cmd==='npm'||cmd==='cargo') {log('build');process.exit(env.RESET_TEST_FAIL===cmd?1:0);}
if(cmd==='restart.sh') {
  log('restart');
  fs.appendFileSync(env.RESET_TEST_LOG,JSON.stringify({event:'restart-env',url:env.ACCOUNTS_DATABASE_URL,galaxy:env.GALAXY_DATA_DIR,bots:env.BOT_PLAYERS,origin:env.APP_ORIGIN})+'\\n');
  process.exit(env.RESET_TEST_FAIL==='restart'?1:0);
}
throw new Error('Unexpected mock command '+cmd);
`;
  for (const name of ["psql", "lsof", "npm", "cargo"]) {
    writeFileSync(join(fixture, "bin", name), mock); chmodSync(join(fixture, "bin", name), 0o755);
  }
  writeFileSync(join(fixture, "scripts/restart.sh"), mock); chmodSync(join(fixture, "scripts/restart.sh"), 0o755);
  // Shadow Bash built-ins only inside the fixture to test graceful shutdown
  // without ever sending a signal to a real PID or waiting for a real server.
  const shellMocks = join(fixture, "shell-mocks.sh");
  writeFileSync(shellMocks, `kill() {
  if [[ "$1" == -TERM ]]; then
    printf '{"event":"stop"}\\n' >> "$RESET_TEST_LOG"
    [[ "\${RESET_TEST_FAIL:-}" != stop ]]
  else
    [[ "$1" == -0 && "\${RESET_TEST_FAIL:-}" == shutdown ]]
  fi
}
sleep() { :; }
`);
  const base = { ...process.env, PATH: `${join(fixture, "bin")}:${dirname(process.execPath)}:${process.env.PATH}`, RESET_TEST_ROOT: fixture };
  for (const key of ["ACCOUNTS_DATABASE_URL", "DATABASE_URL", "APP_ORIGIN", "GALAXY_DATA_DIR", "PORT", "DEVDB_PORT", "MAX_PLAYERS", "BOT_PLAYERS", "BASH_ENV", "ENV", "RESET_TEST_FAIL", "RESET_TEST_SERVER"]) delete base[key];
  let runs = 0;
  function run(args = [], env = {}, input = "") {
    const log = join(fixture, `calls-${++runs}.jsonl`);
    writeFileSync(log, "");
    const result = spawnSync("bash", [join(fixture, "scripts/reset-database.sh"), ...args], {
      cwd: tmpdir(), env: { ...base, RESET_TEST_LOG: log, ...env }, input, encoding: "utf8", timeout: 10000,
    });
    assert.ifError(result.error);
    return { ...result, calls: readFileSync(log, "utf8").trim().split("\n").filter(Boolean).map(line => JSON.parse(line)) };
  }
  function nondestructive(result) {
    assert.ok(result.calls.every(call => call.event === "read"), JSON.stringify(result));
  }
  let result = run(["--help"]);
  assert.equal(result.status, 0); assert.match(result.stdout, /Permanently deletes ALL accounts/); assert.deepEqual(result.calls, []);
  result = run(["--unknown"]); assert.notEqual(result.status, 0); nondestructive(result);
  result = run(["--dry-run"]); assert.equal(result.status, 0, result.stderr); assert.match(result.stdout, /Dry run only/); nondestructive(result);
  result = run(); assert.equal(result.status, 0); assert.match(result.stdout, /Cancelled/); nondestructive(result);
  result = run([], {}, "no\n"); assert.equal(result.status, 0); nondestructive(result);
  for (const env of [
    { ACCOUNTS_DATABASE_URL: "postgres://secret@remote/prod" },
    { DATABASE_URL: "postgres://secret@remote/prod" },
    { APP_ORIGIN: "https://live.example.com" }, { GALAXY_DATA_DIR: "/unrelated/save" },
    { PORT: "8080;echo nope" }, { DEVDB_PORT: "0" }, { MAX_PLAYERS: "0" },
    { RESET_TEST_FAIL: "cluster" }, { RESET_TEST_FAIL: "database" }, { RESET_TEST_FAIL: "listener" },
    { RESET_TEST_FAIL: "inspect" },
  ]) {
    result = run(["--yes"], env); assert.notEqual(result.status, 0, JSON.stringify(env)); nondestructive(result);
    assert.doesNotMatch(result.stderr, /secret@remote/, "database secrets must not be printed");
  }
  mkdirSync(join(fixture, ".devdb/registration-reset.lock"));
  result = run(["--yes"]); assert.notEqual(result.status, 0); nondestructive(result);
  rmSync(join(fixture, ".devdb/registration-reset.lock"), { recursive: true });
  for (const fail of ["npm", "cargo"]) {
    result = run(["--yes"], { RESET_TEST_FAIL: fail }); assert.notEqual(result.status, 0);
    assert.ok(!result.calls.some(call => ["delete", "restart"].includes(call.event)));
  }
  result = run(["--yes"], { RESET_TEST_FAIL: "delete" }); assert.notEqual(result.status, 0);
  assert.ok(!result.calls.some(call => call.event === "restart"));
  result = run(["--yes"], { RESET_TEST_FAIL: "restart" }); assert.notEqual(result.status, 0);
  assert.match(result.stderr, /Accounts were deleted, but galaxy startup failed/);
  const serverEnv = { RESET_TEST_SERVER: "1", BASH_ENV: shellMocks };
  for (const failure of ["listener_change", "stop", "shutdown", "replacement", "cluster_late"]) {
    result = run(["--yes"], { ...serverEnv, RESET_TEST_FAIL: failure });
    assert.notEqual(result.status, 0, failure);
    assert.ok(!result.calls.some(call => ["delete", "restart"].includes(call.event)), failure);
  }
  result = run(["--yes"], serverEnv);
  assert.equal(result.status, 0, result.stderr);
  assert.deepEqual(result.calls.filter(call => call.event !== "read").map(call => call.event),
    ["build", "build", "stop", "delete", "restart", "restart-env"]);
  for (const [args, input] of [[["--yes"], ""], [[], "RESET\n"]]) {
    result = run(args, { BOT_PLAYERS: "8" }, input); assert.equal(result.status, 0, result.stderr);
    const actions = result.calls.filter(call => call.event !== "read");
    assert.deepEqual(actions.map(call => call.event), ["build", "build", "delete", "restart", "restart-env"]);
    const deletion = actions[2];
    assert.ok(deletion.args.includes("-X") && deletion.args.includes("-w") && deletion.args.includes("--single-transaction"));
    assert.match(deletion.args.join(" "), /TRUNCATE TABLE public.account_sessions, public.accounts;/);
    assert.doesNotMatch(deletion.args.join(" "), /CASCADE|DROP/);
    assert.deepEqual(actions[3].args, ["--reset-galaxy"]);
    assert.equal(actions[4].bots, "0");
    assert.equal(actions[4].galaxy, join(fixture, "saves/galaxy-8080"));
    assert.equal(actions[4].url, "postgres://postgres@127.0.0.1:5433/stellar_syndicates");
    assert.match(result.stdout, /Reset complete/);
  }
  console.log(`Reset helper: ${runs} isolated cases passed; no real database or game process was touched.`);
} finally {
  // Only this test's explicitly created temporary fixture is removed.
  rmSync(fixture, { recursive: true, force: true });
}
