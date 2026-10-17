import { expect, test } from "vitest";
import type { JsonConsoleJob, JsonConsoleRunJobs } from "../../types/bencher";
import { duration } from "../reports/format";
import {
	POLL,
	elapsed,
	landed,
	live,
	reportStatus,
	runStatus,
	statusText,
	unfinished,
} from "./status";

const T = Date.parse("2026-10-16T14:51:00Z");
const s = (seconds: number) => T + seconds * 1_000;

const job = (fields: Partial<JsonConsoleJob>) =>
	({ status: "queued", created: T, ...fields }) as JsonConsoleJob;

const jobs = (fields: Partial<JsonConsoleRunJobs>) =>
	({
		queued: 0,
		running: 0,
		failed: 0,
		canceled: 0,
		...fields,
	}) as JsonConsoleRunJobs;

// Kills a running time read to the second past a minute, rounded up, or never
// reaching hours.
test("a running job's time is coarse: seconds, then whole minutes, then hours", () => {
	expect(elapsed(48_900)).toBe("48s");
	expect(elapsed(119_000)).toBe("1m");
	expect(elapsed(59 * 60_000 + 59_000)).toBe("59m");
	expect(elapsed(2 * 3_600_000 + 5 * 60_000)).toBe("2h");
	expect(elapsed(-5_000)).toBe("0s");
});

// Kills a finished report shown with a status, and its time taken from
// anything but its start and end.
test("a report without a job reads as the time it took", () => {
	expect(
		statusText(
			reportStatus({ start_time: T, end_time: s(128) }, s(999), duration),
		),
	).toBe("2m 08s");
});

// Kills each job status read as another, a running time counted from its
// submission once it started or from nothing before, a failed time counted
// from the report's submission rather than its job's start, else its job's
// submission, and a time on a queued or canceled job.
test("a report's job reads as its status, with the time each status carries", () => {
	const report = (fields: Partial<JsonConsoleJob>) =>
		statusText(
			reportStatus(
				{ start_time: s(-60), end_time: s(-60), job: job(fields) },
				s(48),
				duration,
			),
		);
	expect(report({ status: "queued" } as Partial<JsonConsoleJob>)).toBe(
		"Queued",
	);
	expect(
		report({
			status: "running",
			created: s(-600),
			started: s(0),
		} as Partial<JsonConsoleJob>),
	).toBe("Running 48s");
	expect(
		report({ status: "running", created: s(-12) } as Partial<JsonConsoleJob>),
	).toBe("Running 1m");
	expect(
		report({
			status: "failed",
			started: s(-30),
			completed: s(-18),
		} as Partial<JsonConsoleJob>),
	).toBe("Failed 12s");
	expect(
		report({
			status: "failed",
			created: s(-50),
			completed: s(-18),
		} as Partial<JsonConsoleJob>),
	).toBe("Failed 32s");
	expect(report({ status: "failed" } as Partial<JsonConsoleJob>)).toBe(
		"Failed",
	);
	expect(
		report({ status: "canceled", completed: s(0) } as Partial<JsonConsoleJob>),
	).toBe("Canceled");
});

// Kills a run's status that lets a later state hide an earlier one: anything
// running reads Running, anything waiting reads Queued, then a failure, then a
// cancel, and only a run of finished reports reads as its time.
test("a run reads as its most pressing job, else its time", () => {
	const run = (fields?: Partial<JsonConsoleRunJobs>) =>
		statusText(
			runStatus(
				{
					start_time: T,
					end_time: s(848),
					jobs: fields && jobs(fields),
				},
				s(120),
				duration,
			),
		);
	expect(run({ running: 1, queued: 2, failed: 1, since: s(72) })).toBe(
		"Running 48s",
	);
	expect(run({ queued: 1, failed: 1, canceled: 1, since: s(0) })).toBe(
		"Queued",
	);
	expect(run({ failed: 1, canceled: 1 })).toBe("Failed 14m 08s");
	expect(run({ canceled: 2 })).toBe("Canceled");
	expect(run()).toBe("14m 08s");
});

// Kills a dash on a run some of whose reports landed, and counts on one whose
// reports all wait, ran, failed, or were canceled.
test("a run has landed once any of its reports is finished", () => {
	expect(landed(3, undefined)).toBe(true);
	expect(landed(3, jobs({ queued: 1, running: 1, failed: 1 }))).toBe(false);
	expect(landed(3, jobs({ canceled: 1, running: 1 }))).toBe(true);
});

// Kills a poll that keeps asking about jobs that ended, or stops while one
// still waits or runs.
test("a job is unfinished while it waits or runs", () => {
	expect(unfinished(job({ status: "queued" } as Partial<JsonConsoleJob>))).toBe(
		true,
	);
	expect(
		unfinished(job({ status: "running" } as Partial<JsonConsoleJob>)),
	).toBe(true);
	expect(unfinished(job({ status: "failed" } as Partial<JsonConsoleJob>))).toBe(
		false,
	);
	expect(
		unfinished(job({ status: "canceled" } as Partial<JsonConsoleJob>)),
	).toBe(false);
	expect(unfinished(undefined)).toBe(false);
	expect(unfinished(jobs({ queued: 1 }))).toBe(true);
	expect(unfinished(jobs({ running: 1 }))).toBe(true);
	expect(unfinished(jobs({ failed: 2, canceled: 1 }))).toBe(false);
});

// Kills a poll that keeps asking once nothing is unfinished, or never asks.
test("a live query asks again only while its answer shows something unfinished", () => {
	const { refetchInterval } = live(
		(data: { running: boolean }) => data.running,
	);
	expect(refetchInterval({ state: { data: { running: true } } })).toBe(POLL);
	expect(refetchInterval({ state: { data: { running: false } } })).toBe(false);
	expect(refetchInterval({ state: { data: undefined } })).toBe(false);
});
