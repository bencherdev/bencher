import type { JsonReport } from "../../types/bencher";
import type { Api } from "../api";

const START = Date.parse("2026-09-13T21:46:00Z");
const HOUR = 60 * 60 * 1_000;

/** A report as the list endpoint returns it, the `index`th newest, with only what a row reads. */
export const reportFixture = (
	index: number,
	fields: { [K in keyof JsonReport]?: JsonReport[K] | undefined } = {},
): JsonReport =>
	({
		uuid: `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`,
		user: { uuid: "user", name: "Muriel Bagge", slug: "muriel-bagge" },
		branch: {
			name: "main",
			slug: "main",
			head: { version: { number: index, hash: `9c1f2e4${"0".repeat(33)}` } },
		},
		testbed: { name: "ubuntu-latest", slug: "ubuntu-latest" },
		adapter: "json",
		start_time: new Date(START - index * HOUR).toISOString(),
		end_time: new Date(START - index * HOUR + 120_000).toISOString(),
		counts: {
			results: [{ benchmarks: 18, measures: 2, lines: 36 }],
			alerts: { total: 0, active: 0 },
		},
		...fields,
	}) as unknown as JsonReport;

/** Every request a fake API answered, and a way to answer the next ones. */
export const fakeApi = (
	answer: (
		url: URL,
	) =>
		| { data: unknown; total?: number }
		| Promise<{ data: unknown; total?: number }>,
) => {
	const requests: URL[] = [];
	const api: Api = {
		get: async <T>(path: string) => {
			const url = new URL(path, "http://api.test");
			requests.push(url);
			const { data, total } = await answer(url);
			const headers = new Headers();
			if (total !== undefined) {
				headers.set("X-Total-Count", String(total));
			}
			return { data: data as T, headers };
		},
		send: async () => {
			throw new Error("Reports send no changes");
		},
	};
	return { api, requests };
};

/** Watch the page for the Suspense fallback; the returned call stops and says whether it showed. */
export const watchFallback = (root: Node = document.body) => {
	let shown = false;
	const observer = new MutationObserver((records) => {
		for (const record of records) {
			for (const node of record.addedNodes) {
				shown ||= node.textContent === "Suspended";
			}
		}
	});
	observer.observe(root, { childList: true, subtree: true });
	return () => {
		observer.disconnect();
		return shown;
	};
};
