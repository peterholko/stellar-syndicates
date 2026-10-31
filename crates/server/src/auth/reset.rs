//! `server reset-passphrase <login>`: the operator's reset for a registered
//! account. The new passphrase is typed into this terminal twice with echo off,
//! never taken as an argument or logged. The account's session ends, so every
//! browser signed in as it must sign in again.
use std::io::{BufRead, IsTerminal, Write};
use std::process::Command;

use sqlx::PgPool;
use uuid::Uuid;
use zeroize::Zeroizing;

use super::{AuthError, connect_from_env, normalize_login, password};

pub async fn reset_passphrase(login: &str) -> anyhow::Result<()> {
    let login = normalize_login(login).map_err(rejected)?;
    let pool = connect_from_env().await?;
    let account: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM accounts WHERE login=$1 AND NOT is_guest")
            .bind(&login)
            .fetch_optional(&pool)
            .await?;
    let Some((account,)) = account else {
        anyhow::bail!("no registered account has the login {login}");
    };
    // Ctrl-C while echo is off must not leave the terminal silent.
    let interrupt = tokio::spawn(async {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = Command::new("stty").arg("echo").status();
            eprintln!("\nCancelled; nothing changed.");
            std::process::exit(130);
        }
    });
    let passphrase = read_hidden("New passphrase: ")?;
    password::validate(&passphrase).map_err(rejected)?;
    let again = read_hidden("Again: ")?;
    interrupt.abort();
    anyhow::ensure!(*again == *passphrase, "the two entries differ; nothing changed");
    set_passphrase(&pool, account, passphrase).await?;
    println!("Passphrase reset for {login}. Its old sign-in has ended.");
    Ok(())
}

/// Stores the new passphrase's hash and ends the account's session.
pub(super) async fn set_passphrase(
    pool: &PgPool,
    account: Uuid,
    passphrase: Zeroizing<String>,
) -> anyhow::Result<()> {
    let hash = tokio::task::spawn_blocking(move || password::hash(passphrase))
        .await?
        .map_err(rejected)?;
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE accounts SET password_hash=$1,password_changed_at=now() WHERE id=$2")
        .bind(hash)
        .bind(account)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM account_sessions WHERE account_id=$1")
        .bind(account)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}

/// Prompts on stderr and reads one line, with echo off on a terminal. Piped
/// input (scripts, tests) is read as it comes.
fn read_hidden(label: &str) -> anyhow::Result<Zeroizing<String>> {
    let stdin = std::io::stdin();
    let terminal = stdin.is_terminal();
    eprint!("{label}");
    std::io::stderr().flush()?;
    let echo = terminal.then(EchoOff::new).transpose()?;
    let entry = read_entry(&mut stdin.lock());
    drop(echo);
    if terminal {
        eprintln!();
    }
    entry
}

/// One line without its line ending. Spaces stay: the sign-in form never trims
/// a passphrase either.
pub(super) fn read_entry(input: &mut impl BufRead) -> anyhow::Result<Zeroizing<String>> {
    // Room for the longest passphrase, so growing never leaves a copy behind.
    let mut line = Zeroizing::new(String::with_capacity(1024));
    anyhow::ensure!(input.read_line(&mut line)? > 0, "no passphrase given; nothing changed");
    let kept = line.trim_end_matches(['\r', '\n']).len();
    line.truncate(kept);
    Ok(line)
}

/// Terminal echo off until dropped. `stty` acts on the terminal it inherits as
/// stdin, so no unsafe termios calls are needed.
struct EchoOff;

impl EchoOff {
    fn new() -> anyhow::Result<Self> {
        let off = Command::new("stty").arg("-echo").status()?;
        anyhow::ensure!(off.success(), "cannot turn terminal echo off; nothing changed");
        Ok(Self)
    }
}

impl Drop for EchoOff {
    fn drop(&mut self) {
        let _ = Command::new("stty").arg("echo").status();
    }
}

fn rejected(error: AuthError) -> anyhow::Error {
    match error {
        AuthError::BadInput(message) => anyhow::anyhow!(message),
        _ => anyhow::anyhow!("account service unavailable"),
    }
}
