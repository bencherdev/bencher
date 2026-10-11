// @vitest-environment happy-dom
import { describe, expect, test } from "vitest";
import { deferErrors } from "./deferredErrors";

const raise = (error: Error) =>
	window.dispatchEvent(
		new ErrorEvent("error", { error, message: error.message }),
	);

describe("deferErrors", () => {
	// Kills a reporter that loads late and misses what broke before it.
	test("hands over the errors raised before the reporter arrived", () => {
		const deferred = deferErrors(window);
		const first = new Error("first");
		const second = new Error("second");
		raise(first);
		raise(second);

		const captured: unknown[] = [];
		deferred.flush((error) => captured.push(error));

		expect(captured).toEqual([first, second]);
	});

	// Kills a queue that keeps listening and reports an error twice, once
	// through itself and once through the reporter's own handlers.
	test("stops listening once it hands over", () => {
		const deferred = deferErrors(window);
		const captured: unknown[] = [];
		deferred.flush((error) => captured.push(error));

		raise(new Error("after"));
		deferred.flush((error) => captured.push(error));

		expect(captured).toEqual([]);
	});
});
