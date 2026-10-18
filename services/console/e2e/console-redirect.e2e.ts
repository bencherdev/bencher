import { expect, nextPath, seed, signedIn, test } from "./fixtures";

// The member's projects change as other tests run; the admin's do not.
test.use({ storageState: signedIn(seed.admin) });

// Kills a /console that chains requests before it lands, or that lands
// anywhere but the project with the newest report.
test("/console lands a reader with no version memory on the newest project", async ({
	page,
}) => {
	const requests: string[] = [];
	let landed = false;
	page.on("framenavigated", (frame) => {
		if (
			frame === page.mainFrame() &&
			new URL(frame.url()).pathname !== "/console"
		) {
			landed = true;
		}
	});
	page.on("request", (request) => {
		if (!landed && request.url().startsWith(seed.api_url)) {
			requests.push(`${request.method()} ${new URL(request.url()).pathname}`);
		}
	});

	await page.goto("/console");
	await expect(page).toHaveURL(nextPath(seed.admin_newest.slug));

	expect(requests).toEqual([`GET /v0/console/users/${seed.admin.user.uuid}`]);
});
