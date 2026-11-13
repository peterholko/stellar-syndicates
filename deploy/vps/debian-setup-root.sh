#!/usr/bin/env bash
# Root half of a fresh Debian 13 box for Stellar Syndicates
# (docs/vps-debian-setup.md). Copy this folder to the new box with mac.pub
# (the SSH key peter signs in with) next to this script, and run it as root
# from a terminal: it asks for peter's password. setup-peter.sh then does
# peter's half. Safe to run again.
set -euo pipefail
cd "$(dirname "$0")"

[[ $EUID -eq 0 ]] || { echo "run as root" >&2; exit 1; }
. /etc/os-release
[[ ${ID:-} == debian && ${VERSION_ID%%.*} -ge 13 ]] ||
  { echo "needs Debian 13 or newer, this is ${PRETTY_NAME:-unknown}" >&2; exit 1; }
[[ -s mac.pub ]] || { echo "missing mac.pub next to this script" >&2; exit 1; }

SS=/home/peter/stellar-syndicates

step() { printf '\n== %s\n' "$*"; }
# postgres cannot enter /root, so its commands run from /.
pg() { (cd / && runuser -u postgres -- "$@"); }

step "Hostname, locale, clock"
hostnamectl set-hostname stellarsyndicates
grep -q 'vps.stellarsyndicates.com' /etc/hosts ||
  echo '127.0.1.1 vps.stellarsyndicates.com stellarsyndicates' >>/etc/hosts
# Provider images let cloud-init put the panel's hostname back at boot.
[[ -d /etc/cloud/cloud.cfg.d ]] &&
  echo 'preserve_hostname: true' >/etc/cloud/cloud.cfg.d/99-keep-hostname.cfg
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y locales
# The database is en_US.UTF-8; the locale has to exist before PostgreSQL does.
sed -i 's/^# *en_US.UTF-8 UTF-8/en_US.UTF-8 UTF-8/' /etc/locale.gen
locale-gen
update-locale LANG=en_US.UTF-8
export LANG=en_US.UTF-8
timedatectl set-timezone Etc/UTC

step "Packages"
apt-get install -y \
  sudo curl ca-certificates rsync openssl \
  build-essential pkg-config libssl-dev \
  postgresql \
  ufw unattended-upgrades systemd-timesyncd dbus-user-session libpam-systemd
cat >/etc/apt/apt.conf.d/20auto-upgrades <<'EOF'
APT::Periodic::Update-Package-Lists "1";
APT::Periodic::Unattended-Upgrade "1";
EOF

step "Firewall: ssh in, nothing else"
# The game server listens on localhost and is reached through the tunnel,
# which only dials out.
ufw allow 22/tcp
ufw --force enable

step "User peter"
id peter &>/dev/null || useradd -m -s /bin/bash peter
usermod -aG sudo peter
install -d -m 700 -o peter -g peter /home/peter/.ssh
touch /home/peter/.ssh/authorized_keys
while IFS= read -r key || [[ -n $key ]]; do
  [[ -n $key ]] || continue
  grep -qxF "$key" /home/peter/.ssh/authorized_keys || echo "$key" >>/home/peter/.ssh/authorized_keys
done <mac.pub
chown peter: /home/peter/.ssh/authorized_keys
chmod 600 /home/peter/.ssh/authorized_keys
# The tunnel and the game server run as peter's systemd user units; linger
# starts them at boot.
loginctl enable-linger peter
if [[ $(passwd -S peter | awk '{print $2}') != P ]]; then
  echo "Set peter's password (sudo asks for it):"
  passwd peter
fi

step "PostgreSQL role and database"
systemctl enable --now postgresql
for _ in $(seq 1 30); do pg pg_isready -q && break; sleep 1; done
[[ $(pg psql -XtAc "select 1 from pg_roles where rolname='stellar'") == 1 ]] ||
  pg psql -Xq -v ON_ERROR_STOP=1 -c 'CREATE ROLE stellar LOGIN'
[[ $(pg psql -XtAc "select 1 from pg_database where datname='stellar_syndicates'") == 1 ]] ||
  pg createdb -O stellar -E UTF8 -T template0 --locale=en_US.UTF-8 stellar_syndicates
if [[ ! -s $SS/secrets/app.env ]]; then
  # Hex, so it needs no quoting in SQL or in the URL. The statement goes in
  # on stdin to stay out of the process list.
  pw="$(openssl rand -hex 24)"
  pg psql -Xq -v ON_ERROR_STOP=1 <<<"ALTER ROLE stellar PASSWORD '$pw';"
  # The server connects over TCP with the password; prove that path works.
  PGPASSWORD="$pw" psql -X -h 127.0.0.1 -U stellar -d stellar_syndicates -tAc 'select 1' >/dev/null
  install -d -m 755 -o peter -g peter "$SS"
  install -d -m 700 -o peter -g peter "$SS/secrets"
  install -m 600 -o peter -g peter /dev/null "$SS/secrets/app.env"
  printf 'DATABASE_URL=postgres://stellar:%s@127.0.0.1:5432/stellar_syndicates\n' "$pw" >"$SS/secrets/app.env"
  unset pw
fi

step "Hand peter his half"
install -d -m 755 -o peter -g peter /home/peter/vps-setup
cp -r bin systemd-user setup-peter.sh /home/peter/vps-setup/
chown -R peter: /home/peter/vps-setup

step "sshd: keys only, peter only, no root"
# Last, and only once peter can get in by key and use sudo.
[[ -s /home/peter/.ssh/authorized_keys && $(passwd -S peter | awk '{print $2}') == P ]] ||
  { echo "peter has no key or no password; ssh left as it is" >&2; exit 1; }
# sshd keeps the first value it reads, so this file sorts ahead of any the
# provider's image ships (50-cloud-init.conf and the like).
cat >/etc/ssh/sshd_config.d/10-stellarsyndicates.conf <<'EOF'
PermitRootLogin no
PasswordAuthentication no
KbdInteractiveAuthentication no
PubkeyAuthentication yes
AllowUsers peter
X11Forwarding no
EOF
sshd -t
effective="$(sshd -T)"
for want in 'permitrootlogin no' 'passwordauthentication no' 'kbdinteractiveauthentication no' 'allowusers peter'; do
  grep -qx "$want" <<<"$effective" && continue
  echo "sshd is not using '$want'; something earlier in its config overrides it:" >&2
  grep -rnE '^\s*(Include|PermitRootLogin|PasswordAuthentication|KbdInteractiveAuthentication|AllowUsers)' \
    /etc/ssh/sshd_config /etc/ssh/sshd_config.d >&2
  exit 1
done
systemctl reload ssh

step "Done"
echo "$PRETTY_NAME, kernel $(uname -r)"
pg psql -XtAc 'select version()' | cut -d' ' -f1-2
id peter
ufw status | sed -n 1,6p
echo "New SSH sessions: peter only, by key. Root and passwords are refused."
echo "Next, as peter: bash ~/vps-setup/setup-peter.sh"
