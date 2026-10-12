import { expect, test } from "vitest";
import { FILTERED, scrubber } from "./scrub";

const USER_KEY = `bencher_user_${"Ab3".repeat(10)}`;
const PROJECT_KEY = `bencher_run_${"cD4".repeat(10)}`;
const UUID = "0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f";

// Kills error reporting that sends a pasted API key anywhere in an event: a
// message, a nested field, a list, or an object that refers to itself.
test("filters every API key out of an event", () => {
	const event: Record<string, unknown> = {
		message: `Bearer ${USER_KEY} was refused`,
		exception: {
			values: [{ value: `Invalid key ${PROJECT_KEY}`, type: "Error" }],
		},
		breadcrumbs: [
			{ category: "console", data: { arguments: [USER_KEY, 7, null] } },
		],
		extra: { headers: { Authorization: `Bearer ${USER_KEY}` } },
	};
	event.self = event;

	const scrubbed = scrubber([])(event);
	const text = JSON.stringify(scrubbed, (key, value) =>
		key === "self" ? undefined : value,
	);
	expect(text).not.toContain("bencher_user_A");
	expect(text).not.toContain("bencher_run_c");
	expect(scrubbed.message).toBe(`Bearer ${FILTERED} was refused`);
	expect(scrubbed.exception).toEqual({
		values: [{ value: `Invalid key ${FILTERED}`, type: "Error" }],
	});
	expect(scrubbed.breadcrumbs).toEqual([
		{ category: "console", data: { arguments: [FILTERED, 7, null] } },
	]);
	expect(scrubbed.self).toBe(scrubbed);
});

// Kills error reporting that sends a secret it was given, in any case, in an
// event's URL, transaction, request, breadcrumbs, or spans.
test("filters each given secret out of an event, in any case", () => {
	const event = {
		transaction: `GET /v0/oauth/authorizations/${UUID}`,
		request: {
			url: `https://bencher.dev/auth/authorize?request=${UUID.toUpperCase()}`,
			query_string: `request=${UUID}`,
		},
		breadcrumbs: [
			{
				category: "fetch",
				data: {
					url: `https://api.bencher.dev/v0/oauth/authorizations/${UUID}`,
				},
			},
		],
		spans: [{ description: `GET /v0/oauth/authorizations/${UUID}` }],
		tags: { kept: "0b5c4f6e" },
	};

	const scrubbed = scrubber([UUID])(event);
	expect(JSON.stringify(scrubbed).toLowerCase()).not.toContain(UUID);
	expect(scrubbed.request.url).toBe(
		`https://bencher.dev/auth/authorize?request=${FILTERED}`,
	);
	expect(scrubbed.tags.kept).toBe("0b5c4f6e");
});
