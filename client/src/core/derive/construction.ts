import type { BuildState } from "../../protocol";

/** The straight line a reported job's progress follows: `base` percent at sim
 * time `at`, rising `slope` percent a second after it (never before). */
export interface ProgressLine {
  at: number;
  base: number;
  slope: number;
}

/** A job's progress line from the received job, never a current
 * recipe/assignment. A workforce pause freezes the earned fraction instead of
 * moving the start. */
export function progressLine(job: BuildState): ProgressLine | null {
  if (job.work) {
    const { fraction, per_second, at_time } = job.work;
    if (![fraction, per_second, at_time].every(Number.isFinite) || per_second < 0) return null;
    return { at: at_time, base: fraction * 100, slope: job.queued ? 0 : per_second * 100 };
  }
  if (job.queued) return { at: 0, base: 0, slope: 0 };
  // Non-ship and legacy reports retain their known fixed span. Never invent a
  // start for older snapshots; their ETA can still be shown without a bar.
  const start = job.start_time;
  const total = start == null || job.complete_time == null ? 0 : job.complete_time - start;
  return start != null && Number.isFinite(start) && Number.isFinite(total) && total > 0
    ? { at: start, base: 0, slope: 100 / total } : null;
}

/** A progress line's percent at sim time `now`. */
export function progressAt(line: ProgressLine, now: number): number {
  return Math.max(0, Math.min(100, line.base + Math.max(0, now - line.at) * line.slope));
}

/** Estimated progress (percent) of the received job at `now`. */
export function buildProgress(job: BuildState, now: number): number | null {
  const line = progressLine(job);
  return line ? progressAt(line, now) : null;
}

/** §live-progress (Peter, Oct 27 2026): a bar carries its line, so each frame
 * redraws it from the render clock instead of it jumping when its panel
 * redraws (shell/liveprogress.ts). Empty without a line. */
export function liveProgressAttr(line: ProgressLine | null): string {
  return line ? ` data-live-progress="${line.at} ${line.base} ${line.slope}"` : "";
}

/** §yard-build: a hull under construction on the planet's yard pad carries
 * its line the same way; the frame tick sets its `--build` (0–1) and tag. */
export function liveBuildAttr(line: ProgressLine | null): string {
  return line ? ` data-live-build="${line.at} ${line.base} ${line.slope}"` : "";
}

/** A bar's line back from its `data-live-progress` value. */
export function readLiveProgress(value: string | undefined): ProgressLine | null {
  const [at, base, slope] = (value ?? "").split(" ").map(Number);
  return [at, base, slope].every(Number.isFinite) ? { at, base, slope } : null;
}
