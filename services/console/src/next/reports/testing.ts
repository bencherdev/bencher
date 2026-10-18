import type { Api } from "../api";

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
