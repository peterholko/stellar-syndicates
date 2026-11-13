// §migration-status (Nov 1 2026, Peter: "Yes fix it"): one own world's
// immigration as both shells' Population tabs show it: its housing, and one
// line naming the next landing or what holds the next liner. The server
// serves the allocation pass's own gates (`BodyView.migration`, v82); this
// only words them. Short text; the why is the tooltip.

import type { BodyView, MigrationHold } from "../../protocol";

export interface MigrationStatus {
  text: string;
  tip: string;
  warn: boolean;
}

export const MIGRATION_HOLDS: Record<MigrationHold, MigrationStatus> = {
  closed: { text: "Closed", tip: "Choose Managed, Open or Priority to take settlers.", warn: false },
  unsettled: { text: "Uninhabited", tip: "Liners land only where people live.", warn: false },
  blockaded: { text: "Blockaded", tip: "No liner is sent while a blockade holds the system.", warn: true },
  food: { text: "Needs food", tip: "Liners come only while the system is Well Supplied.", warn: true },
  housing: { text: "Housing full", tip: "Build or upgrade a Habitat.", warn: true },
  crew_free: { text: "Crew idle", tip: "Managed waits until every crew is posted.", warn: false },
  staffed: { text: "Posts staffed", tip: "Managed brings settlers for empty crew posts. Build or upgrade, or choose Open.", warn: false },
};

/** "7,000 / 12,500": people here against the housing the allocation reads;
 * null without a served status (a rival's world). */
export function housingText(body: Pick<BodyView, "population" | "migration">): string | null {
  return body.migration ? `${people(body.population)} / ${people(body.migration.housing)}` : null;
}

/** The status line at sim time `now`; null without a served status. */
export function migrationStatus(body: Pick<BodyView, "migration">, now: number): MigrationStatus | null {
  const m = body.migration;
  if (!m) return null;
  if (m.liners > 0) {
    const left = m.lands_at == null ? null : m.lands_at - now;
    const lands = left === null ? "lands" : left > 30 ? `lands in ${minutes(left)}` : "landing";
    return {
      text: m.liners === 1 ? `Liner ${lands}` : `${m.liners} liners · first ${lands}`,
      tip: "From the Wormhole Hub.",
      warn: false,
    };
  }
  if (m.hold) return MIGRATION_HOLDS[m.hold];
  if (m.next_at != null) {
    const left = m.next_at - now;
    return { text: left > 30 ? `Next liner in ${minutes(left)}` : "Next liner due", tip: "The Hub sends the next liner here.", warn: false };
  }
  return { text: "Queued", tip: "Another of your worlds gets the next liner.", warn: false };
}

function minutes(secs: number): string {
  return `${Math.ceil(secs / 60)}m`;
}

function people(millions: number): string {
  return Math.round(millions * 1_000_000).toLocaleString("en-US");
}
