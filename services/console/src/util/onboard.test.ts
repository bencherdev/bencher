// @vitest-environment happy-dom
import { afterEach, describe, expect, test } from "vitest";
import {
	getOnboardProjectKey,
	removeOnboardProjectKey,
	setOnboardProjectKey,
} from "./onboard";

afterEach(() => {
	sessionStorage.clear();
});

describe("onboard sessionStorage", () => {
	test("set and get project key", () => {
		setOnboardProjectKey("my-key");
		expect(getOnboardProjectKey()).toBe("my-key");
	});

	test("get returns null when not set", () => {
		expect(getOnboardProjectKey()).toBeNull();
	});

	test("remove clears the key", () => {
		setOnboardProjectKey("my-key");
		removeOnboardProjectKey();
		expect(getOnboardProjectKey()).toBeNull();
	});

	test("set overwrites existing key", () => {
		setOnboardProjectKey("first");
		setOnboardProjectKey("second");
		expect(getOnboardProjectKey()).toBe("second");
	});
});
