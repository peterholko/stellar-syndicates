import type { BuildState } from "../../protocol";

/** Estimated progress from the received job, never a current recipe/assignment.
 * A workforce pause freezes the earned fraction instead of moving the start. */
export function buildProgress(job: BuildState, now: number): number | null {
  if (job.work) {
    const { fraction, per_second, at_time } = job.work;
    if (![fraction, per_second, at_time].every(Number.isFinite) || per_second < 0) return null;
    return Math.max(0, Math.min(100, (fraction + (job.queued ? 0 : Math.max(0, now - at_time) * per_second)) * 100));
  }
  if (job.queued) return 0;
  // Non-ship and legacy reports retain their known fixed span. Never invent a
  // start for older snapshots; their ETA can still be shown without a bar.
  const start = job.start_time;
  const total = start == null || job.complete_time == null ? 0 : job.complete_time - start;
  return start != null && Number.isFinite(start) && Number.isFinite(total) && total > 0
    ? Math.max(0, Math.min(100, (now - start) / total * 100)) : null;
}

/** One visible group per planet. The server orders waiting jobs by receipt;
 * preserve that order and never turn a queued job active from a client estimate. */
export function buildsByPlanet(builds: BuildState[]): { bodyId: number; jobs: BuildState[] }[] {
  const groups = new Map<number, BuildState[]>();
  for (const job of builds) {
    const jobs = groups.get(job.body_id) ?? [];
    jobs.push(job);
    groups.set(job.body_id, jobs);
  }
  return [...groups].sort(([a], [b]) => a - b).map(([bodyId, jobs]) => ({
    bodyId, jobs: jobs.sort((a, b) => Number(!!a.queued) - Number(!!b.queued)),
  }));
}
