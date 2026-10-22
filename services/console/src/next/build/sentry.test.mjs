import { expect, test } from "vitest";
import { sentryEnvironment } from "./sentry.mjs";

// Kills a build that reports every deploy as production.
test("a build for a Cloudflare environment reports that environment", () => {
	expect(sentryEnvironment("dev")).toBe("dev");
});

// Kills `??` for `||`: CI selects the top level with an empty value, not none.
test("a build for the top level reports production", () => {
	expect(sentryEnvironment("")).toBe("production");
	expect(sentryEnvironment(undefined)).toBe("production");
});
