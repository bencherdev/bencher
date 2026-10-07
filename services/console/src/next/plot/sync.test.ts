import { expect, test } from "vitest";
import { returnSyncKey, takeSyncKey } from "./sync";

// Kills a key minted per mount, which grows uPlot's registry with every expanded row.
test("hands a returned key to the next plot", () => {
	const key = takeSyncKey();
	returnSyncKey(key);
	expect(takeSyncKey()).toBe(key);
});

// Kills two mounted plots sharing a key, which would sync unrelated cursors.
test("never hands one key to two plots at once", () => {
	const first = takeSyncKey();
	const second = takeSyncKey();
	expect(second).not.toBe(first);
	returnSyncKey(first);
	returnSyncKey(second);
});
