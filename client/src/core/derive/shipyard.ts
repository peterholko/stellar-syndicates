// §shipyard-view (Peter, Oct 21 2026): the hulls a system's yards are building,
// for the Shipyard panel (Deck pop-out, mobile sheet) and its 3D viewer.
import type { ShipKind, SystemStateView } from "../../protocol";
import { HULL_YARD, isYardKind, type YardKind } from "../../shipyard-scene";
import { progressAt, progressLine, type ProgressLine } from "./construction";
import { shipKindLabel } from "./fleet";
import { YARD_TITLE } from "./market";
import { isHullJob, yardStatus } from "./worldbuild";

export interface YardJob {
  /** Stable across reports: the job's id, else its place among the system's builds. */
  key: string;
  kind: ShipKind;
  yard: YardKind;
  title: string;
  /** "Shipyard · Freya 12". */
  where: string;
  /** 0–1 built; null while it waits for a slip, or without a served clock. */
  progress: number | null;
  /** Its progress line, for a bar that redraws every frame (§live-progress). */
  line: ProgressLine | null;
  /** The yard line's word on it: time left, "waiting for a slip", … */
  status: string;
  active: boolean;
  /** Its place among the system's served builds: later is newer. */
  order: number;
}

/** The system's hulls under construction or waiting, building ones first, in
 * the served order. Only what the arrived report says. */
export function yardJobs(system: SystemStateView | undefined, now: number): YardJob[] {
  if (!system) return [];
  const jobs = system.builds.flatMap((job, index) => {
    if (!isHullJob(job.key)) return [];
    const kind = job.key as ShipKind;
    const yard = isYardKind(job.yard) ? job.yard : HULL_YARD[kind] ?? "shipyard";
    const body = system.bodies.find(entry => entry.id === job.body_id);
    const active = !job.queued && job.complete_time != null;
    const line = active ? progressLine(job) : null;
    const progress = line ? progressAt(line, now) : null;
    return [{
      key: job.id !== undefined ? `job:${job.id}` : `at:${index}`,
      kind, yard, title: shipKindLabel(kind),
      where: `${YARD_TITLE[yard] ?? "Shipyard"}${body ? ` · ${body.name}` : ""}`,
      progress: progress === null ? null : progress / 100,
      line,
      status: yardStatus(job, now),
      active,
      order: index,
    }];
  });
  return [...jobs.filter(job => job.active), ...jobs.filter(job => !job.active)];
}

/** Seconds past its due report after which a hull order is presumed refused,
 * as a structure order is (deck empire.ts). */
const SILENT_AFTER_S = 15;
/** An order this far past due is forgotten: the yard shows what it has. */
const FORGET_AFTER_S = 600;

export type YardFallback = { state: "sending" } | { state: "silent" } | { state: "launched" } | null;

/** What the yard shows for the hull it was opened on when the report holds no
 * job of that kind. Seen building since: it launched. Else an order is on its
 * way until its report is due (two light legs), then maybe refused. */
export function yardFallback(seen: boolean, sentAt: number | null, delay: number, now: number): YardFallback {
  if (seen) return { state: "launched" };
  if (sentAt === null) return null;
  const due = sentAt + 2 * delay + 3;
  if (now <= due + SILENT_AFTER_S) return { state: "sending" };
  return now <= due + FORGET_AFTER_S ? { state: "silent" } : null;
}

/** The hull the viewer shows: the one picked, else the newest of the kind just
 * ordered, else the first building. */
export function pickYardJob(jobs: readonly YardJob[], picked: string | null, ordered?: string | null): YardJob | null {
  const newest = ordered ? jobs.filter(job => job.kind === ordered).sort((a, b) => b.order - a.order)[0] : undefined;
  return jobs.find(job => job.key === picked) ?? newest ?? jobs[0] ?? null;
}
