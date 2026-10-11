import { gzipSync } from "node:zlib";
import type { Page } from "@playwright/test";

/** What a load or a navigation cost, as the speed ceilings measure it. */
export interface Cost {
	/**
	 * API requests answered before the first contentful paint: data the first
	 * paint could have waited on. Which starts first, a request or the paint,
	 * is a race between script and renderer, so starts are not counted.
	 */
	apiAnswersBeforePaint: number;
	/** API requests started since the mark. */
	apiRequests: number;
	/** The longest chain of API requests: a request starting after another finished is the next round. */
	apiRounds: number;
	/** Cumulative layout shift since the page loaded. */
	cls: number;
	/**
	 * Same origin JavaScript started since the mark and, on a load, before the
	 * load event, gzipped at level 9: what the page needs to run.
	 */
	jsBytes: number;
	/** Same origin JavaScript started after the load event: what waits for idle. */
	deferredJsBytes: number;
	/** Same origin stylesheets started since the mark, each a request the first paint waits on. */
	stylesheets: number;
	/** The HTML document as the server sent it, gzipped at level 9. */
	htmlBytes: number;
	/**
	 * Every inline script in that document, uncompressed: what the browser
	 * parses and runs before it can paint.
	 */
	inlineScriptBytes: number;
	/**
	 * Modules the page loaded before the load event that its document did not
	 * name, each found only once the module importing it arrived.
	 */
	lateModules: number;
}

declare global {
	interface Window {
		__layoutShift?: number;
	}
}

/** Sum the layout shifts the reader did not cause, from the first paint on. */
export const observeLayoutShift = (page: Page) =>
	page.addInitScript(() => {
		window.__layoutShift = 0;
		new PerformanceObserver((list) => {
			for (const entry of list.getEntries()) {
				const shift = entry as PerformanceEntry & {
					value: number;
					hadRecentInput: boolean;
				};
				if (!shift.hadRecentInput) {
					window.__layoutShift = (window.__layoutShift ?? 0) + shift.value;
				}
			}
		}).observe({ type: "layout-shift", buffered: true });
	});

/** A point on the page's own clock to measure a navigation from. */
export const mark = (page: Page) => page.evaluate(() => performance.now());

/** Wait until the page has loaded and the browser has had an idle moment. */
export const loadedAndIdle = (page: Page) =>
	page.evaluate(
		() =>
			new Promise<void>((resolve) => {
				const idle = () => requestIdleCallback(() => resolve());
				if (document.readyState === "complete") {
					idle();
				} else {
					window.addEventListener("load", idle, { once: true });
				}
			}),
	);

export const measure = async (
	page: Page,
	apiUrl: string,
	since = 0,
): Promise<Cost> => {
	const timing = await page.evaluate(
		([api, from]) => {
			const paint = performance.getEntriesByName("first-contentful-paint")[0];
			const navigation = performance.getEntriesByType("navigation")[0] as
				| PerformanceNavigationTiming
				| undefined;
			const resources = performance
				.getEntriesByType("resource")
				.filter(
					(entry) => entry.startTime >= from,
				) as PerformanceResourceTiming[];
			return {
				paint: paint?.startTime ?? Number.POSITIVE_INFINITY,
				loaded: navigation?.loadEventStart || Number.POSITIVE_INFINITY,
				api: resources
					.filter((entry) => entry.name.startsWith(api))
					.map((entry) => ({ start: entry.startTime, end: entry.responseEnd })),
				scripts: resources
					.filter((entry) => {
						const url = new URL(entry.name);
						return (
							url.origin === location.origin && url.pathname.endsWith(".js")
						);
					})
					.map((entry) => ({ url: entry.name, start: entry.startTime })),
				styles: resources
					.map((entry) => new URL(entry.name))
					.filter(
						(url) =>
							url.origin === location.origin && url.pathname.endsWith(".css"),
					)
					.map((url) => url.href),
				cls: window.__layoutShift ?? 0,
			};
		},
		[apiUrl, since] as const,
	);

	// What the server sent, before any script added links of its own.
	const served = await (await page.request.get(page.url())).body();
	const html = served.toString("utf8");
	const inlineScripts = [
		...html.matchAll(/<script\b(?![^>]*\bsrc=)[^>]*>([\s\S]*?)<\/script>/g),
	].map(([, body]) => body ?? "");
	const named = new Set(
		[...html.matchAll(/<(?:link|script)\b[^>]*\b(?:href|src)="([^"]+\.js)"/g)]
			.map(([, path]) => path)
			.filter((path): path is string => path !== undefined)
			.map((path) => new URL(path, page.url()).href),
	);

	const gzipped = async (urls: string[]) => {
		let bytes = 0;
		for (const url of new Set(urls)) {
			const body = await (await page.request.get(url)).body();
			bytes += gzipSync(body, { level: 9 }).length;
		}
		return bytes;
	};
	// A navigation after the load counts everything it loads.
	const beforeLoad = (script: { start: number }) =>
		since > 0 || script.start < timing.loaded;
	const critical = timing.scripts.filter(beforeLoad);
	const deferred = timing.scripts.filter((script) => !beforeLoad(script));

	return {
		apiAnswersBeforePaint: timing.api.filter(
			(request) => request.end < timing.paint,
		).length,
		apiRequests: timing.api.length,
		apiRounds: rounds(timing.api),
		cls: timing.cls,
		jsBytes: await gzipped(critical.map((script) => script.url)),
		deferredJsBytes: await gzipped(deferred.map((script) => script.url)),
		stylesheets: new Set(timing.styles).size,
		htmlBytes: gzipSync(served, { level: 9 }).length,
		inlineScriptBytes: inlineScripts.reduce(
			(bytes, body) => bytes + Buffer.byteLength(body),
			0,
		),
		lateModules: new Set(
			critical.map((script) => script.url).filter((url) => !named.has(url)),
		).size,
	};
};

const rounds = (requests: { start: number; end: number }[]) => {
	const sorted = [...requests].sort((a, b) => a.start - b.start);
	const depth: number[] = [];
	for (const [i, request] of sorted.entries()) {
		const before = sorted
			.slice(0, i)
			.map((earlier, j) =>
				earlier.end <= request.start ? (depth[j] ?? 0) : 0,
			);
		depth.push(1 + Math.max(0, ...before));
	}
	return Math.max(0, ...depth);
};
