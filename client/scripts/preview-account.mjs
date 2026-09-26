// Isolated visual QA: real shared form and shell styles, no account API or game socket.
// Run: node client/scripts/preview-account.mjs (?mode=profile checks stacked dialogs).
import { createServer } from "node:http";
import { readFileSync } from "node:fs";
import { resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import ts from "typescript";
import vm from "node:vm";

const root = fileURLToPath(new URL("../", import.meta.url));
const compiled = ts.transpileModule(readFileSync(resolve(root, "src/shell/account.ts"), "utf8"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS },
}).outputText;
const accountUI = {};
vm.runInNewContext(compiled, { exports: accountUI, require: () => ({}) });

const server = createServer((req, res) => {
  const url = new URL(req.url, "http://localhost");
  if (url.pathname === "/") {
    const mobile = url.searchParams.get("shell") === "mobile";
    const prefix = mobile ? "m" : "deck";
    const profile = url.searchParams.get("mode") === "profile";
    const session = profile ? { id: "preview-only", corporation_name: "Preview corporation", is_guest: true } : null;
    const api = `const previewOnly=async()=>{throw new Error('Preview only — no account was created.');};
      const accounts={session:async()=>(${JSON.stringify(session)}),register:previewOnly,signIn:previewOnly,guest:previewOnly,signOut:previewOnly,completeRegistration:previewOnly};`;
    res.setHeader("Content-Type", "text/html; charset=utf-8");
    res.end(`<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
      <title>Stellar Charter · isolated preview</title>
      <link rel="stylesheet" href="/styles/tokens.css"><link rel="stylesheet" href="/styles/account.css">
      <link rel="stylesheet" href="/styles/${mobile ? "mobile" : "deck"}.css"></head>
      <body><div class="${mobile ? "m-shell" : "deck"}"><section class="${mobile ? "m-join" : "deck-overlay deck-join"}" aria-labelledby="${prefix}-join-title">
        ${accountUI.accountBackdrop(prefix)}<form id="${prefix}-join-form" class="${prefix}-join__card account-card" data-account-mode="signin">${accountUI.accountFields(prefix)}</form>
      </section>${accountUI.accountProfile(prefix)}<button id="${prefix}-profile-open" hidden>Profile</button></div>
      <script>${api}const ui={};((exports,require)=>{${compiled}})(ui,()=>({accounts}));
        ui.bindAccountForm('${prefix}',{net:{disconnect(){}}},new AbortController().signal);
        ${url.searchParams.get("mode") === "signin" ? "" : "document.querySelector('[data-account-toggle]').click();"}
        ${profile ? `document.getElementById('${prefix}-profile-open').click();` : ""}
      </script></body></html>`);
    return;
  }
  const base = url.pathname.startsWith("/styles/") ? resolve(root, "src/styles") : resolve(root, "public");
  const relative = url.pathname.startsWith("/styles/") ? url.pathname.slice(8) : `.${url.pathname}`;
  const path = resolve(base, decodeURIComponent(relative));
  if (!path.startsWith(base + sep)) { res.writeHead(403); res.end(); return; }
  try {
    res.setHeader("Content-Type", path.endsWith(".css") ? "text/css" : path.endsWith(".webp") ? "image/webp" : path.endsWith(".svg") ? "image/svg+xml" : "application/octet-stream");
    res.end(readFileSync(path));
  } catch { res.writeHead(404); res.end(); }
});
server.listen(0, "127.0.0.1", () => console.log(`Charter preview: http://127.0.0.1:${server.address().port} (add ?shell=mobile or ?mode=signin)`));
