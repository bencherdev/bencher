import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "./fixtures";

/** The Sentry DSN a production build of the console reports to. */
const productionDsn = () =>
	/^PUBLIC_SENTRY_DSN=(.*)$/m
		.exec(readFileSync(".env.production", "utf8"))?.[1]
		?.trim() ?? "";

// Kills a test console built with the production DSN, which sends every error
// and trace an end-to-end run raises to production.
test("the console under test carries no Sentry DSN", () => {
	const dsn = productionDsn();
	expect(dsn).not.toBe("");
	const carrying = readdirSync("dist", { recursive: true, encoding: "utf8" })
		.filter((file) => /\.(m?js|html)$/.test(file))
		.filter((file) => readFileSync(join("dist", file), "utf8").includes(dsn));
	expect(carrying).toEqual([]);
});
