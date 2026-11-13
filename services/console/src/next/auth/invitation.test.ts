import { describe, expect, test } from "vitest";
import { readInvitation } from "./invitation";

const SECRET = "A".repeat(43);

describe("readInvitation", () => {
	// Kills an invitation read by its secret anywhere but the request's body,
	// where no server log or Referer keeps it, and a secret of the wrong shape
	// sent to the API, whose refusal would carry no reason.
	test("sends a secret of its shape in the body alone, and refuses any other", async () => {
		const sent: { url: string; init: RequestInit | undefined }[] = [];
		const fetch = async (url: RequestInfo | URL, init?: RequestInit) => {
			sent.push({ url: String(url), init });
			return new Response(JSON.stringify({ email: "grace@example.com" }));
		};
		const read = await readInvitation(SECRET, fetch as typeof globalThis.fetch);
		expect(read.ok).toBe(true);
		expect(sent).toHaveLength(1);
		expect(sent[0]?.url).toBe("/api/v0/auth/invite");
		expect(sent[0]?.init?.method).toBe("POST");
		expect(JSON.parse(String(sent[0]?.init?.body))).toEqual({ secret: SECRET });

		for (const broken of ["", "a.b.c", `${SECRET}A`, `${"A".repeat(42)}=`]) {
			const refused = await readInvitation(
				broken,
				fetch as typeof globalThis.fetch,
			);
			expect(refused, broken).toEqual({
				ok: false,
				refused: {
					status: 400,
					code: "invalid_invite",
					retryAfter: undefined,
					triesLeft: undefined,
				},
			});
		}
		expect(sent).toHaveLength(1);
	});
});
