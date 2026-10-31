//! §social (R5, Oct 22 2026): corporations talk to each other — one channel
//! per syndicate, mail between corporations, mute and report, and who in the
//! syndicate is online.
//!
//! This is galaxy state on the server, never the simulation: it checkpoints
//! with the galaxy and resets with it. The Authority's relay carries text
//! instantly; light delay is flavour in the client only. Times are wall-clock
//! Unix seconds, which is what "seen 2 h ago" means to a person.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use sim::{PlayerId, SyndicateId};

/// Lines a syndicate channel keeps.
pub const CHANNEL_HISTORY: usize = 200;
/// Mails a corporation keeps in each of its inbox and sent folders.
pub const MAILBOX_KEEP: usize = 100;
pub const CHAT_MAX_CHARS: usize = 400;
pub const SUBJECT_MAX_CHARS: usize = 80;
pub const MAIL_MAX_CHARS: usize = 2_000;
/// Lines and mails one corporation may send a minute, together.
pub const SENDS_PER_MINUTE: usize = 8;
/// Reports kept for the operators.
pub const REPORTS_KEPT: usize = 500;

/// One line on a syndicate channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatLine {
    pub id: u64,
    pub author: PlayerId,
    pub author_name: String,
    pub text: String,
    pub at: i64,
}

/// One mail; the sender keeps a copy in `sent`, the recipient in `inbox`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mail {
    pub id: u64,
    pub from: PlayerId,
    pub from_name: String,
    pub to: PlayerId,
    pub to_name: String,
    pub subject: String,
    pub body: String,
    pub at: i64,
    #[serde(default)]
    pub read: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Mailbox {
    #[serde(default)]
    pub inbox: VecDeque<Mail>,
    #[serde(default)]
    pub sent: VecDeque<Mail>,
}

/// A line or mail a corporation reported, for the operators.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub reporter: PlayerId,
    pub kind: String,
    pub id: u64,
    pub author: Option<PlayerId>,
    pub text: String,
    pub reason: String,
    pub at: i64,
}

/// One syndicate member as the channel shows them.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct MemberPresence {
    pub player: PlayerId,
    pub name: String,
    pub online: bool,
    /// When they were last connected (None: never, or online now).
    pub last_seen: Option<i64>,
}

/// §settle-near-friends (R7): an invite link that holds a home slot near its
/// sender. The sim keeps the hold under `key`; the code is the link's secret.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Invite {
    pub code: String,
    pub key: u64,
    pub by: PlayerId,
    /// Sim time the hold lapses.
    pub until: f64,
}

/// One of the sender's open invites.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct InviteView {
    pub code: String,
    /// Sim time the hold lapses.
    pub until: f64,
    /// The held home system's name.
    pub near: String,
}

/// What a corporation reads: its syndicate's channel, its mail, who in the
/// syndicate is online, whom it has muted, and its open invites. Muted
/// authors are left out.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct SocialView {
    pub channel: Vec<ChatLine>,
    pub inbox: Vec<Mail>,
    pub sent: Vec<Mail>,
    pub unread: u32,
    pub members: Vec<MemberPresence>,
    pub muted: Vec<PlayerId>,
    pub invites: Vec<InviteView>,
}

/// Invite codes: no 0/O, 1/I/L.
const CODE_ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
const CODE_LEN: usize = 8;

fn invite_code() -> String {
    use rand_core::{OsRng, RngCore};
    (0..CODE_LEN).map(|_| CODE_ALPHABET[(OsRng.next_u32() as usize) % CODE_ALPHABET.len()] as char).collect()
}

/// A typed or pasted code, as stored.
pub fn normalize_code(code: &str) -> String {
    code.trim().to_ascii_uppercase()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Social {
    #[serde(default)]
    pub channels: BTreeMap<SyndicateId, VecDeque<ChatLine>>,
    #[serde(default)]
    pub mail: BTreeMap<PlayerId, Mailbox>,
    #[serde(default)]
    pub mutes: BTreeMap<PlayerId, BTreeSet<PlayerId>>,
    /// When each corporation was last connected.
    #[serde(default)]
    pub last_seen: BTreeMap<PlayerId, i64>,
    #[serde(default)]
    pub reports: VecDeque<Report>,
    #[serde(default)]
    pub next_id: u64,
    /// §settle-near-friends (R7): open invites by code.
    #[serde(default)]
    pub invites: BTreeMap<String, Invite>,
    /// Recent sends per corporation, for the rate limit (not saved).
    #[serde(skip)]
    sends: BTreeMap<PlayerId, VecDeque<i64>>,
    /// Bumped on every change, so a view is rebuilt only when it may differ.
    #[serde(skip)]
    pub version: u64,
}

/// Trimmed text without control characters (mail keeps line breaks), cut to
/// `max` characters; None when nothing is left.
pub fn clean(text: &str, max: usize, lines: bool) -> Option<String> {
    let kept: String = text
        .chars()
        .filter(|c| !c.is_control() || (lines && *c == '\n'))
        .take(max)
        .collect();
    let kept = kept.trim().to_string();
    (!kept.is_empty()).then_some(kept)
}

impl Social {
    fn next(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    /// May `who` send now? Counts this send if so.
    fn allow(&mut self, who: PlayerId, now: i64) -> bool {
        let recent = self.sends.entry(who).or_default();
        while recent.front().is_some_and(|t| now - *t >= 60) {
            recent.pop_front();
        }
        if recent.len() >= SENDS_PER_MINUTE {
            return false;
        }
        recent.push_back(now);
        true
    }

    /// Say `text` on `syndicate`'s channel.
    pub fn say(&mut self, syndicate: SyndicateId, author: PlayerId, author_name: &str, text: &str, now: i64)
        -> Result<(), &'static str>
    {
        self.version += 1;
        let text = clean(text, CHAT_MAX_CHARS, false).ok_or("nothing to send")?;
        if !self.allow(author, now) {
            return Err("too many messages; wait a minute");
        }
        let id = self.next();
        let channel = self.channels.entry(syndicate).or_default();
        channel.push_back(ChatLine { id, author, author_name: author_name.into(), text, at: now });
        while channel.len() > CHANNEL_HISTORY {
            channel.pop_front();
        }
        Ok(())
    }

    /// Mail `to` from `from`. Returns the mail's id.
    #[allow(clippy::too_many_arguments)]
    pub fn send_mail(&mut self, from: PlayerId, from_name: &str, to: PlayerId, to_name: &str, subject: &str,
        body: &str, now: i64) -> Result<u64, &'static str>
    {
        self.version += 1;
        if from == to {
            return Err("you can't mail yourself");
        }
        let subject = clean(subject, SUBJECT_MAX_CHARS, false).unwrap_or_else(|| "(no subject)".into());
        let body = clean(body, MAIL_MAX_CHARS, true).ok_or("the mail is empty")?;
        if !self.allow(from, now) {
            return Err("too many messages; wait a minute");
        }
        let id = self.next();
        let mail = Mail { id, from, from_name: from_name.into(), to, to_name: to_name.into(), subject, body, at: now,
            read: false };
        let keep = |folder: &mut VecDeque<Mail>, mail: Mail| {
            folder.push_back(mail);
            while folder.len() > MAILBOX_KEEP {
                folder.pop_front();
            }
        };
        keep(&mut self.mail.entry(from).or_default().sent, Mail { read: true, ..mail.clone() });
        keep(&mut self.mail.entry(to).or_default().inbox, mail);
        Ok(id)
    }

    pub fn mark_read(&mut self, player: PlayerId, id: u64) {
        self.version += 1;
        if let Some(mail) = self.mail.get_mut(&player).and_then(|b| b.inbox.iter_mut().find(|m| m.id == id)) {
            mail.read = true;
        }
    }

    /// Delete a mail from `player`'s inbox or sent folder (the other side keeps
    /// its copy).
    pub fn delete_mail(&mut self, player: PlayerId, id: u64) {
        self.version += 1;
        if let Some(box_) = self.mail.get_mut(&player) {
            box_.inbox.retain(|m| m.id != id);
            box_.sent.retain(|m| m.id != id);
        }
    }

    pub fn mute(&mut self, player: PlayerId, target: PlayerId, on: bool) {
        self.version += 1;
        let set = self.mutes.entry(player).or_default();
        if on && target != player {
            set.insert(target);
        } else {
            set.remove(&target);
        }
    }

    /// Report a channel line (`kind` "chat") or a received mail ("mail"). The
    /// reported text is copied, so it outlives the channel's history.
    pub fn report(&mut self, reporter: PlayerId, syndicate: Option<SyndicateId>, kind: &str, id: u64, reason: &str,
        now: i64) -> Result<(), &'static str>
    {
        self.version += 1;
        let (author, text) = match kind {
            "chat" => syndicate.and_then(|s| self.channels.get(&s)).and_then(|c| c.iter().find(|l| l.id == id))
                .map(|l| (l.author, l.text.clone())),
            "mail" => self.mail.get(&reporter).and_then(|b| b.inbox.iter().find(|m| m.id == id))
                .map(|m| (m.from, format!("{}: {}", m.subject, m.body))),
            _ => None,
        }.ok_or("nothing to report")?;
        let reason = clean(reason, SUBJECT_MAX_CHARS, false).unwrap_or_default();
        self.reports.push_back(Report { reporter, kind: kind.into(), id, author: Some(author), text, reason, at: now });
        while self.reports.len() > REPORTS_KEPT {
            self.reports.pop_front();
        }
        Ok(())
    }

    /// §settle-near-friends (R7): a new invite from `by`, holding until `until`.
    pub fn create_invite(&mut self, by: PlayerId, until: f64) -> Invite {
        let code = loop {
            let code = invite_code();
            if !self.invites.contains_key(&code) {
                break code;
            }
        };
        self.next_id += 1;
        let invite = Invite { code: code.clone(), key: self.next_id, by, until };
        self.invites.insert(code, invite.clone());
        self.version += 1;
        invite
    }

    pub fn invite(&self, code: &str) -> Option<&Invite> {
        self.invites.get(&normalize_code(code))
    }

    /// Remove `code`: revoked by its sender (`by`), or used (`None`).
    pub fn take_invite(&mut self, code: &str, by: Option<PlayerId>) -> Option<Invite> {
        let code = normalize_code(code);
        if self.invites.get(&code).is_none_or(|i| by.is_some_and(|by| i.by != by)) {
            return None;
        }
        self.version += 1;
        self.invites.remove(&code)
    }

    /// Drop invites whose hold has lapsed.
    pub fn prune_invites(&mut self, now: f64) {
        let before = self.invites.len();
        self.invites.retain(|_, i| i.until > now);
        if self.invites.len() != before {
            self.version += 1;
        }
    }

    /// Unread mail from corporations `player` has not muted.
    pub fn unread(&self, player: PlayerId) -> u32 {
        let muted = self.mutes.get(&player);
        self.mail.get(&player).map_or(0, |b| b.inbox.iter()
            .filter(|m| !m.read && !muted.is_some_and(|set| set.contains(&m.from))).count() as u32)
    }

    /// What `player` reads, given its syndicate's channel and its members
    /// (player, name, online now).
    pub fn view_for(&self, player: PlayerId, syndicate: Option<SyndicateId>, members: &[(PlayerId, String, bool)])
        -> SocialView
    {
        let muted = self.mutes.get(&player);
        let shown = |author: PlayerId| !muted.is_some_and(|m| m.contains(&author));
        let channel = syndicate.and_then(|s| self.channels.get(&s))
            .map(|c| c.iter().filter(|l| shown(l.author)).cloned().collect()).unwrap_or_default();
        let box_ = self.mail.get(&player);
        let inbox: Vec<Mail> = box_.map(|b| b.inbox.iter().rev().filter(|m| shown(m.from)).cloned().collect())
            .unwrap_or_default();
        let sent = box_.map(|b| b.sent.iter().rev().cloned().collect()).unwrap_or_default();
        SocialView {
            channel,
            unread: inbox.iter().filter(|m| !m.read).count() as u32,
            inbox,
            sent,
            members: members.iter().map(|(id, name, online)| MemberPresence {
                player: *id,
                name: name.clone(),
                online: *online,
                last_seen: (!*online).then(|| self.last_seen.get(id).copied()).flatten(),
            }).collect(),
            muted: muted.map(|m| m.iter().copied().collect()).unwrap_or_default(),
            invites: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: PlayerId = PlayerId(1);
    const B: PlayerId = PlayerId(2);
    const S: SyndicateId = SyndicateId(7);

    #[test]
    fn the_channel_keeps_its_history_and_mutes_hide_an_author() {
        let mut social = Social::default();
        social.say(S, A, "Aurora", "  hello\u{7}  ", 100).unwrap();
        social.say(S, B, "Borealis", "hi", 101).unwrap();
        assert_eq!(social.say(S, A, "Aurora", "   ", 102), Err("nothing to send"));
        let view = social.view_for(A, Some(S), &[(A, "Aurora".into(), true), (B, "Borealis".into(), false)]);
        assert_eq!(view.channel.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["hello", "hi"]);
        social.last_seen.insert(B, 90);
        social.mute(A, B, true);
        let view = social.view_for(A, Some(S), &[(A, "Aurora".into(), true), (B, "Borealis".into(), false)]);
        assert_eq!(view.channel.len(), 1, "a muted author's lines are hidden");
        assert_eq!(view.members[1].last_seen, Some(90));
        assert_eq!(view.members[0].last_seen, None, "online now");
        for i in 0..(CHANNEL_HISTORY as i64 + 20) {
            social.sends.clear();
            social.say(S, B, "Borealis", "spam", 200 + i).unwrap();
        }
        assert_eq!(social.channels[&S].len(), CHANNEL_HISTORY);
    }

    #[test]
    fn mail_reaches_the_inbox_and_the_rate_limit_holds() {
        let mut social = Social::default();
        let id = social.send_mail(A, "Aurora", B, "Borealis", "Trade?", "Ore for alloys.\nThoughts?", 100).unwrap();
        assert_eq!(social.send_mail(A, "Aurora", A, "Aurora", "x", "y", 100), Err("you can't mail yourself"));
        let view = social.view_for(B, None, &[]);
        assert_eq!((view.unread, view.inbox[0].body.as_str()), (1, "Ore for alloys.\nThoughts?"));
        social.mark_read(B, id);
        assert_eq!(social.view_for(B, None, &[]).unread, 0);
        assert_eq!(social.view_for(A, None, &[]).sent.len(), 1);
        social.report(B, None, "mail", id, "spam", 101).unwrap();
        assert_eq!(social.reports[0].author, Some(A));
        social.delete_mail(B, id);
        assert!(social.view_for(B, None, &[]).inbox.is_empty());
        assert_eq!(social.view_for(A, None, &[]).sent.len(), 1, "the sender keeps its copy");
        let mut sent = 0;
        for _ in 0..20 {
            if social.say(S, A, "Aurora", "x", 500).is_ok() {
                sent += 1;
            }
        }
        assert_eq!(sent, SENDS_PER_MINUTE, "a minute's sends, then a wait");
    }
}
