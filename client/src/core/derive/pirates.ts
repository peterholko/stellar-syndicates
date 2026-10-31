/** Mirrors pirate.rs's public site templates. Call ONLY with served scout intel
 * or an arrived contract tier — this is not a detector or live garrison query. */
export function pirateSite(tier: number, den = false, surname?: string): { title: string; art: string | null; goal: string } | null {
  if (tier <= 0) return null;
  // §charter-v4: the founding privateer's den, named by its log (§vendetta:
  // by its family).
  if (den) return { title: surname ? `${surname} den` : "Privateer den", art: null,
    goal: "Three damaged raiders · no defenses · destroyed for good · plunder for a Freighter" };
  if (tier >= 5) return { title: "Regional stronghold", art: "/art/pirate-sites/stronghold.png",
    goal: "Cruiser-led assault · permanent settlement opportunity + recoverable plunder" };
  if (tier === 4) return { title: "Fortified pirate depot", art: "/art/pirate-sites/depot.png",
    goal: "Destroyer group · clear permanently, then recover plunder with a Freighter" };
  return { title: `Privateer hideout · tier ${tier}`, art: null,
    goal: "Suppress the base to interrupt its raids. This hideout can rebuild." };
}
