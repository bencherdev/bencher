import {
	USER_KEY,
	expect,
	nextPath,
	seed,
	signedIn,
	signedOut,
	test,
} from "./fixtures";

const signInWayBack = (path: string) =>
	`/auth/login?${new URLSearchParams({ back: Buffer.from(path).toString("base64") })}`;

test.describe("signed out", () => {
	test.use({ storageState: signedOut });

	// Kills a console that draws a private project for nobody, and a sign in
	// that forgets where the reader was going.
	test("the private project sends the reader to sign in with the way back", async ({
		page,
	}) => {
		const path = nextPath(seed.projects.private.slug, "runs");
		await page.goto(path);
		await expect(page).toHaveURL(signInWayBack(path));
	});

	// Kills validators whose WASM never loads, which leaves Log in disabled for good.
	test("a valid email enables Log in", async ({ page }) => {
		await page.goto("/auth/login");
		const logIn = page.getByRole("button", { name: "Log in", exact: true });
		// The form replaces its placeholder once it loads, so fill until it takes.
		await expect(async () => {
			await page
				.getByPlaceholder("email@example.com")
				.fill("reader@example.com");
			await expect(logIn).toBeEnabled({ timeout: 1_000 });
		}).toPass();
	});
});

test.describe("signed in", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a client that treats a refused token as an empty project, and a
	// reader left signed in with it.
	test("a token the API refuses sends the reader to sign in with the way back", async ({
		page,
	}) => {
		await page.route(`${seed.api_url}/v0/console/projects/**`, (route) =>
			route.fulfill({ status: 401, json: { message: "Unauthorized" } }),
		);
		const path = nextPath(seed.projects.hashbrown.slug, "alerts");
		await page.goto(path);
		await expect(page).toHaveURL(signInWayBack(path));
		expect(
			await page.evaluate((key) => localStorage.getItem(key), USER_KEY),
		).toBeNull();
	});
});

test.describe("with a badly signed token", () => {
	const [header, payload] = seed.member.token.split(".");
	test.use({
		storageState: signedIn({
			...seed.member,
			token: `${header}.${payload}.${"A".repeat(43)}`,
		}),
	});

	// Kills a client that reads the API's 400 for a token it cannot validate as
	// an empty project instead of a reader who must sign in again.
	test("the reader is sent to sign in with the way back", async ({ page }) => {
		const path = nextPath(seed.projects.hashbrown.slug, "alerts");
		await page.goto(path);
		await expect(page).toHaveURL(signInWayBack(path));
	});
});
