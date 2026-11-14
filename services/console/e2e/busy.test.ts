import { expect, test } from "vitest";
import { whileBusy } from "./busy";

/** What `node:sqlite` throws for a database another connection holds locked. */
const busy = (errcode = 5) =>
	Object.assign(new Error("database is locked"), {
		code: "ERR_SQLITE_ERROR",
		errcode,
	});

// Kills a write that gives up on the first lock, or on a lock it reads as
// another error by its extended code.
test("a write runs again while the database is locked", () => {
	const answers = [busy(), busy(517), "written"];
	let runs = 0;
	const write = () => {
		const answer = answers[runs++];
		if (answer instanceof Error) {
			throw answer;
		}
		return answer;
	};
	expect(whileBusy(write)).toBe("written");
	expect(runs).toBe(3);
});

// Kills a write that runs again on any error, or past its deadline.
test("a write stops at another error, and at its deadline", () => {
	let runs = 0;
	const failed = new Error("no such table: session");
	expect(() =>
		whileBusy(() => {
			runs++;
			throw failed;
		}),
	).toThrow(failed);
	expect(runs).toBe(1);

	runs = 0;
	let clock = 0;
	expect(() =>
		whileBusy(
			() => {
				runs++;
				if (runs > 3) {
					throw new Error("ran past its deadline");
				}
				clock += 1_000;
				throw busy();
			},
			2_000,
			() => clock,
		),
	).toThrow("database is locked");
	expect(runs).toBe(2);
});
