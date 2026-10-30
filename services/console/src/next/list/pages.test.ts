import { QueryClient } from "@tanstack/solid-query";
import { expect, test } from "vitest";
import { type Page, putRow } from "./pages";

// Kills a row put back into a list that already holds it, which then draws
// twice and counts one too many.
test("putRow leaves a list that already holds the row as it was", () => {
	const client = new QueryClient();
	const key = ["keys", 16, 1];
	const page: Page<{ uuid: string }> = {
		rows: [{ uuid: "a" }, { uuid: "b" }],
		total: 2,
		page: 1,
	};
	client.setQueryData(key, page);
	putRow(client, ["keys"], { uuid: "b" });
	expect(client.getQueryData(key)).toEqual(page);
	putRow(client, ["keys"], { uuid: "c" });
	expect(client.getQueryData(key)).toEqual({
		rows: [{ uuid: "c" }, { uuid: "a" }, { uuid: "b" }],
		total: 3,
		page: 1,
	});
});
