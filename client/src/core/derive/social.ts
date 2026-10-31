// §social (R5): shared helpers for the syndicate channel and mail, both shells.

import type { SocialView } from "../../protocol";

/** "now", "4m", "3h", "2d" since `at` (Unix seconds). */
export function ago(at: number, nowSecs = Date.now() / 1000): string {
  const s = Math.max(0, nowSecs - at);
  if (s < 60) return "now";
  if (s < 3_600) return `${Math.floor(s / 60)}m`;
  if (s < 86_400) return `${Math.floor(s / 3_600)}h`;
  return `${Math.floor(s / 86_400)}d`;
}

const SEEN_KEY = "stellarSyndicatesChannelSeen";

/** The last channel line this browser showed. */
export function channelSeenId(): number {
  try { return Number(localStorage.getItem(SEEN_KEY)) || 0; } catch { return 0; }
}

export function markChannelSeen(social: SocialView | null): void {
  const last = social?.channel.at(-1)?.id ?? 0;
  if (last <= channelSeenId()) return;
  try { localStorage.setItem(SEEN_KEY, String(last)); } catch { /* private window: the badge just repeats */ }
}

/** Others' channel lines since the last one shown. */
export function unseenLines(social: SocialView | null, me: string | null): number {
  const seen = channelSeenId();
  return social?.channel.filter(line => line.id > seen && line.author !== me).length ?? 0;
}

/** The Comms badge: unread mail and unseen channel lines. */
export function commsBadge(social: SocialView | null, me: string | null): number {
  return (social?.unread ?? 0) + unseenLines(social, me);
}

/** §settle-near-friends (R7): open invites a corporation may hold at once. */
export const INVITES_MAX = 3;

/** The link a friend opens. */
export function inviteLink(code: string): string {
  return `${location.origin}/?invite=${encodeURIComponent(code)}`;
}

const INVITE_KEY = "stellarSyndicatesInvite";

/** The invite code this browser arrived with (`?invite=`), kept until landing. */
export function storedInvite(): string | null {
  try {
    const fromUrl = new URLSearchParams(location.search).get("invite")?.trim();
    if (fromUrl) {
      localStorage.setItem(INVITE_KEY, fromUrl);
      const url = new URL(location.href);
      url.searchParams.delete("invite");
      history.replaceState(history.state, "", url);
    }
    return localStorage.getItem(INVITE_KEY);
  } catch { return null; }
}

export function clearStoredInvite(): void {
  try { localStorage.removeItem(INVITE_KEY); } catch { /* nothing kept */ }
}

/** A member's presence, tersely. */
export function presenceText(member: { online: boolean; last_seen: number | null }): string {
  return member.online ? "online" : member.last_seen === null ? "offline" : `seen ${ago(member.last_seen)} ago`;
}
