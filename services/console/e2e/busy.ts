/** SQLite's primary result code for a database another connection holds locked. */
const SQLITE_BUSY = 5;
/**
 * How long a write to the API's database keeps waiting out the API's own
 * writes, which a loaded run can hold for longer than one busy timeout, and
 * well inside a test's own timeout.
 */
const DEADLINE_MS = 20_000;

/** `write`'s result, run again each time the database is locked, until `deadline`. */
export const whileBusy = <T>(
	write: () => T,
	deadline = Date.now() + DEADLINE_MS,
	now: () => number = Date.now,
): T => {
	for (;;) {
		try {
			return write();
		} catch (error) {
			if (!isBusy(error) || now() >= deadline) {
				throw error;
			}
		}
	}
};

const isBusy = (error: unknown) =>
	typeof error === "object" &&
	error !== null &&
	"errcode" in error &&
	typeof error.errcode === "number" &&
	(error.errcode & 0xff) === SQLITE_BUSY;
