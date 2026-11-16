import { expect, signedOut, test } from "./fixtures";
import { embedLink } from "./public";

test.use({ storageState: signedOut });

const FRAME_ANCESTORS = "frame-ancestors 'none'";

// Kills a page under /auth/ or /console/ that another site can frame, one
// that was served as a file, a redirect, or a path no page serves, and a
// framing rule that reaches the pages around them, the embed above all, which
// a site frames on purpose.
test("auth and console pages refuse every frame, and the embed takes one", async ({
	request,
}) => {
	for (const [path, status] of [
		["/auth/login", 200],
		["/auth/email", 200],
		["/auth/authorize", 200],
		["/auth/no-such-page", 404],
		["/console", 200],
		["/console/projects", 301],
		["/console/users/settings", 200],
		["/console/onboard/token", 200],
		["/console/no-such-page", 404],
	] as const) {
		const answer = await request.get(path, { maxRedirects: 0 });
		expect(answer.status(), path).toBe(status);
		expect(answer.headers()["content-security-policy"], path).toBe(
			FRAME_ANCESTORS,
		);
	}
	for (const path of [embedLink("no-such-project"), "/", "/docs/"]) {
		const answer = await request.get(path, { maxRedirects: 0 });
		expect(answer.status(), path).toBeLessThan(400);
		expect(answer.headers()["content-security-policy"], path).toBeUndefined();
	}
});

// Kills a not found page under /auth/ or /console/ whose status line says
// other than Not Found.
test("a path no auth or console page serves is not found", async ({
	request,
}) => {
	for (const path of ["/auth/no-such-page", "/console/no-such-page"]) {
		const answer = await request.get(path, { maxRedirects: 0 });
		expect(answer.status(), path).toBe(404);
		expect(answer.statusText(), path).toBe("Not Found");
	}
});
