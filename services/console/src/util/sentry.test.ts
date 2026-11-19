import { httpHeadersToSpanAttributes } from "@sentry/astro";
import { expect, test } from "vitest";
import { dataCollection } from "./sentry";

// Kills a console server that hands Sentry the pending sign in cookie a console on plain HTTP
// sets, whose name no snippet of Sentry's own matches.
test("Sentry gets no Bencher cookie's value", () => {
	const attributes = httpHeadersToSpanAttributes(
		{ cookie: "bencher_pending=PENDING; bencher_session=SESSION; theme=dark" },
		dataCollection as Parameters<typeof httpHeadersToSpanAttributes>[1],
	);
	expect(attributes["http.request.header.cookie"]).toEqual([
		"bencher_pending=[Filtered]",
		"bencher_session=[Filtered]",
		"theme=dark",
	]);
});
