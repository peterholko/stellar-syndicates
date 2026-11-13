# Debian setup

A fresh Debian 13 install of the VPS that runs the Stellar Syndicates game
server. The scripts are in [deploy/vps](../deploy/vps). How the public
addresses reach the box is in [front-door.md](front-door.md).

## What the box is

- Hostname `stellarsyndicates`, address `vps.stellarsyndicates.com`
  (172.245.142.11).
- It runs one thing: the game server on `127.0.0.1:18080`, reached only through
  the Cloudflare tunnel `stellar-syndicates`. Cloudflare serves the client and
  holds the certificates, so the box needs neither.
- The only port open to the internet is SSH.
- It starts empty: a new galaxy, no accounts, and whatever build is deployed
  first.

`stellarsyndicates.com` itself cannot be the SSH address: that name points at
Cloudflare, which serves the game and does not pass SSH through.

## Have ready

1. **Tunnel token** for the `stellar-syndicates` tunnel, saved as
   `~/tunnel-token` on the Mac. If it is lost: Cloudflare dashboard → Zero
   Trust → Networks → Tunnels → `stellar-syndicates` → Edit; the install
   command shown there contains it (`eyJ…`).
2. **DNS record** in the `stellarsyndicates.com` zone: `vps` → A →
   `172.245.142.11`, **DNS only** (grey cloud). Until it exists, use the IP
   wherever a command says `vps.stellarsyndicates.com`.

## Steps

Run the commands on the Mac.

### 1. Reinstall

Provider panel → reinstall as **Debian 13, 64-bit**. Keep the root password.
It has to be 13: the script refuses anything older. Whatever hostname the panel
sets, the root script renames the box `stellarsyndicates` and keeps it that way.

### 2. Root setup

`deploy/vps/mac.pub` must hold the public key peter signs in with
(`cp ~/.ssh/id_ed25519.pub deploy/vps/mac.pub`; it is git-ignored).

```bash
COPYFILE_DISABLE=1 tar --no-xattrs -czf - -C ~/projects/stellar-syndicates/deploy/vps . | ssh root@172.245.142.11 'mkdir -p /root/vps-setup && tar -xzf - -C /root/vps-setup'
```

```bash
ssh -t root@172.245.142.11 'bash /root/vps-setup/debian-setup-root.sh'
```

It asks one thing: a password for `peter` (sudo uses it). It finishes by
locking SSH down: only `peter`, only by key, no root, no passwords. It checks
that sshd really took those settings before reloading. If it stops part-way,
run it again.

### 3. Peter's setup

```bash
ssh -i ~/.ssh/id_ed25519 peter@vps.stellarsyndicates.com 'bash ~/vps-setup/setup-peter.sh'
```

### 4. Tunnel

```bash
scp -i ~/.ssh/id_ed25519 ~/tunnel-token peter@vps.stellarsyndicates.com:stellar-syndicates/secrets/tunnel-token && ssh -i ~/.ssh/id_ed25519 peter@vps.stellarsyndicates.com 'chmod 600 ~/stellar-syndicates/secrets/tunnel-token && ~/stellar-syndicates/bin/start-tunnel.sh'
```

### 5. Deploy

Ask Claude to deploy to production. That builds the client, copies the server
source to `~/stellar-syndicates/releases/<rev>`, builds it on the box, starts it
with `bin/start-app.sh` (a new galaxy generates on first start), then publishes
the client and both Workers to Cloudflare.

Afterwards register your login on stellarsyndicates.com, put it in
`ADMIN_LOGINS` in `~/stellar-syndicates/bin/run-app.sh`, and restart
(`bin/stop-app.sh`, `bin/start-app.sh`).

### 6. Reboot test

```bash
ssh -t -i ~/.ssh/id_ed25519 peter@vps.stellarsyndicates.com 'sudo reboot'
```

A minute later this should print 200:

```bash
curl -s -o /dev/null -w "%{http_code}\n" https://play.stellarsyndicates.com/healthz
```

## What the scripts set up

| Script | Runs as | Does |
|---|---|---|
| `debian-setup-root.sh` | root | hostname, UTC, `en_US.UTF-8`; packages (PostgreSQL 17, build tools); firewall; user `peter` (sudo, linger, SSH key); the database role, database and password file; SSH hardening; unattended upgrades |
| `setup-peter.sh` | peter | `~/stellar-syndicates` folders and `bin/` scripts; cloudflared; the game-server, tunnel and tunnel-update user services; rustup |

## Layout on the box

`~/stellar-syndicates`:

| Path | Holds |
|---|---|
| `bin/` | `run-app.sh` (settings), `start-app.sh [--reset-galaxy]`, `stop-app.sh`, `start-tunnel.sh`, `stop-tunnel.sh`, `update-tunnel.sh` |
| `releases/<rev>/`, `current` | each deployed build; `current` links to the live one |
| `data/galaxy-18080/` | galaxy saves (every 15 minutes and on stop) |
| `secrets/` | `app.env` (database URL), `tunnel-token` |
| `logs/app.log` | server output |

Services (`systemctl --user`, started at boot through linger):

| Unit | Does |
|---|---|
| `stellar-syndicates.service` | the game server; restarts after a crash; a stop waits for the shutdown save |
| `stellar-syndicates-tunnel@18081`, `@18083` | two tunnel connectors, so one can restart while the other carries traffic |
| `stellar-syndicates-tunnel-update.timer` | daily cloudflared update, one connector at a time |

Database: role `stellar`, database `stellar_syndicates`, localhost only.

## Known gaps

- The scripts have not run on a real Debian box. They pass a syntax check and
  every package name exists in Debian 13.
- SSH is keys-only: add any other device's key to `~/.ssh/authorized_keys`
  before you need it. The provider's console still gives a root login.
