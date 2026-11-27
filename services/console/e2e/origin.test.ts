import { expect, test } from "vitest";
import { strayToApi } from "./origin";

const API = "http://localhost:6610";

// Kills an exemption for any top navigation to the API, for any request to its
// OAuth pages, or for a path that only starts like them.
test("only a top navigation to the API's OAuth pages keeps to the console's origin", () => {
	expect(strayToApi(new URL(`${API}/oauth/authorize?client_id=c`), true)).toBe(
		false,
	);
	expect(strayToApi(new URL(`${API}/v0/projects`), true)).toBe(true);
	expect(strayToApi(new URL(`${API}/oauth/authorize`), false)).toBe(true);
	expect(strayToApi(new URL(`${API}/oauthx/authorize`), true)).toBe(true);
});
