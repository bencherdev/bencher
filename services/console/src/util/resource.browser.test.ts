// @vitest-environment happy-dom
import { afterEach, describe, expect, test } from "vitest";
import { BENCHER_TITLE, setPageTitle } from "./resource";

afterEach(() => {
	document.title = "";
});

describe("setPageTitle", () => {
	test("sets document title with Bencher suffix", () => {
		setPageTitle("Projects");
		expect(document.title).toBe(`Projects | ${BENCHER_TITLE}`);
	});

	test("sets document title to Bencher title for undefined", () => {
		setPageTitle(undefined);
		expect(document.title).toBe(BENCHER_TITLE);
	});

	test("does not re-set when title is already correct", () => {
		const expected = `Projects | ${BENCHER_TITLE}`;
		document.title = expected;
		setPageTitle("Projects");
		expect(document.title).toBe(expected);
	});
});
