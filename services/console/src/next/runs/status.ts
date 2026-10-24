import type {
	ConsoleJobStatus,
	JsonConsoleJob,
	JsonConsoleRun,
	JsonConsoleRunJobs,
} from "../../types/bencher";
import type { Api } from "../api";

/** A runner job's state as the console names it, compared by value so the generated enum stays out of the bundle. */
export type JobKind = `${ConsoleJobStatus}`;

/** What a status cell shows: a finished run or report's time, else its runner job's state and the time that state carries. */
export interface Status {
	kind?: JobKind;
	time?: string;
}

export const LABELS: Record<JobKind, string> = {
	queued: "Queued",
	running: "Running",
	failed: "Failed",
	canceled: "Canceled",
};

/** How long a job has run, coarsely: seconds, then whole minutes, then whole hours. */
export const elapsed = (ms: number) => {
	const seconds = Math.max(0, Math.floor(ms / 1_000));
	if (seconds < 60) {
		return `${seconds}s`;
	}
	const minutes = Math.floor(seconds / 60);
	return minutes < 60 ? `${minutes}m` : `${Math.floor(minutes / 60)}h`;
};

/** A report's status: its runner job's, else the time it took, as `duration` reads it. */
export const reportStatus = (
	report: {
		job?: JsonConsoleJob | undefined;
		start_time: number;
		end_time: number;
	},
	now: number,
	duration: (ms: number) => string,
): Status => {
	const job = report.job;
	if (!job) {
		return { time: duration(report.end_time - report.start_time) };
	}
	const kind = kindOf(job);
	if (kind === "running") {
		return { kind, time: elapsed(now - (job.started ?? job.created)) };
	}
	// A failed job never rewrites its report's times, so it is timed as it ran.
	if (kind === "failed" && job.completed !== undefined) {
		return {
			kind,
			time: duration(job.completed - (job.started ?? job.created)),
		};
	}
	return { kind };
};

/** A run's status: Running while any report runs, Queued while any waits, then Failed or Canceled if any ended so, else the time it took, as `duration` reads it. */
export const runStatus = (
	run: {
		jobs?: JsonConsoleRunJobs | undefined;
		start_time: number;
		end_time: number;
	},
	now: number,
	duration: (ms: number) => string,
): Status => {
	const jobs = run.jobs;
	const time = duration(run.end_time - run.start_time);
	if (jobs?.running) {
		return { kind: "running", time: elapsed(now - (jobs.since ?? now)) };
	}
	if (jobs?.queued) {
		return { kind: "queued" };
	}
	if (jobs?.failed) {
		return { kind: "failed", time };
	}
	return jobs?.canceled ? { kind: "canceled" } : { time };
};

export const statusText = ({ kind, time }: Status) =>
	[kind && LABELS[kind], time].filter(Boolean).join(" ");

/** Whether any of a run's `reports` has its results in, so its counts show rather than a dash. */
export const landed = (reports: number, jobs: JsonConsoleRunJobs | undefined) =>
	!jobs || reports > jobs.queued + jobs.running + jobs.failed + jobs.canceled;

/** Whether a report's job, or any of a run's, still waits or runs: the page asks again while one does. */
export const unfinished = (
	job: JsonConsoleJob | JsonConsoleRunJobs | undefined,
) =>
	job !== undefined &&
	("status" in job
		? kindOf(job) === "queued" || kindOf(job) === "running"
		: job.queued + job.running > 0);

const kindOf = (job: JsonConsoleJob) => job.status as string as JobKind;

/** How often a page asks again while it shows a runner job that waits or runs, as the empty project's card asks for its first report. */
export const POLL = 5_000;

/**
 * A query whose answer can show something unfinished: it asks again every
 * `POLL` while it does, and the cache never stores such an answer, so a poll
 * writes nothing and a return never paints a stale status.
 */
export const live = <T>(unfinished: (data: T) => boolean) => ({
	refetchInterval: ({ state }: { state: { data: T | undefined } }) =>
		state.data !== undefined && unfinished(state.data) ? POLL : false,
	meta: { unfinished },
});

/** How many of a run's reports an opened run lists; its page lists the rest. */
export const RUN_REPORTS = 8;

/** The first of a run's reports in start order, as an opened run lists them, on the Runs list and blank Explore alike. */
export const runReportsQuery = (api: Api, slug: string, run: string) => ({
	queryKey: ["console", "run-reports", slug, run],
	queryFn: async () =>
		(
			await api.get<JsonConsoleRun>(
				`/v0/console/projects/${encodeURIComponent(slug)}/runs/${encodeURIComponent(run)}?per_page=${RUN_REPORTS}`,
			)
		).data,
	...live((run: JsonConsoleRun) => unfinished(run.jobs)),
});
