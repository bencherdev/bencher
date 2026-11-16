import type { Page } from "@playwright/test";

/**
 * Answers what the console sends to its test Sentry DSN as Sentry would, or
 * blocks it as a browser's blocker does, and keeps every envelope.
 */
export const sentry = async (page: Page, answer: "accept" | "block") => {
	const envelopes: string[] = [];
	await page.route("http://127.0.0.1:9/**", (route) => {
		envelopes.push(route.request().postData() ?? "");
		return answer === "accept"
			? route.fulfill({ status: 200, json: {} })
			: route.abort("blockedbyclient");
	});
	return envelopes;
};
