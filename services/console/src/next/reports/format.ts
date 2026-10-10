import type { JsonReport } from "../../types/bencher";

const SECOND = 1_000;
const MINUTE = 60 * SECOND;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** How long ago `time` was, in the largest whole unit that fits. */
export const relativeTime = (time: number, now: number) => {
	const ago = now - time;
	if (ago < MINUTE) {
		return "just now";
	}
	if (ago < HOUR) {
		return `${Math.floor(ago / MINUTE)}m ago`;
	}
	if (ago < DAY) {
		return `${Math.floor(ago / HOUR)}h ago`;
	}
	const days = Math.floor(ago / DAY);
	if (days < 14) {
		return `${days}d ago`;
	}
	if (days < 60) {
		return `${Math.floor(days / 7)}w ago`;
	}
	if (days < 365) {
		return `${Math.floor((days * 12) / 365)}mo ago`;
	}
	return `${Math.floor(days / 365)}y ago`;
};

const ABSOLUTE: Intl.DateTimeFormatOptions = {
	month: "short",
	day: "numeric",
	year: "numeric",
	hour: "2-digit",
	minute: "2-digit",
	hourCycle: "h23",
};

/** The date and the 24 hour time, in `timeZone` or the reader's own. */
export const absoluteTime = (time: number, timeZone?: string) =>
	new Intl.DateTimeFormat("en-US", { ...ABSOLUTE, timeZone }).format(time);

const pad = (n: number) => String(n).padStart(2, "0");

/** How long a run took: seconds, then minutes and seconds, then hours and minutes. */
export const duration = (ms: number) => {
	const seconds = Math.max(0, Math.floor(ms / SECOND));
	if (seconds < 60) {
		return `${seconds}s`;
	}
	const minutes = Math.floor(seconds / 60);
	if (minutes < 60) {
		return `${minutes}m ${pad(seconds % 60)}s`;
	}
	return `${Math.floor(minutes / 60)}h ${pad(minutes % 60)}m`;
};

export const shortHash = (hash: string | undefined) => hash?.slice(0, 7);

/** Who ran a report: the project key that created it, else the person. */
export const creator = (report: JsonReport) => {
	if (report.project_key) {
		return { kind: "key", name: report.project_key.name } as const;
	}
	if (report.user) {
		return { kind: "user", name: report.user.name } as const;
	}
	return undefined;
};

/** A report's lines: every iteration reports the same lines, so the most any reported. */
export const lines = (report: JsonReport) =>
	Math.max(
		0,
		...(report.counts?.results ?? []).map((iteration) => iteration.lines ?? 0),
	);
