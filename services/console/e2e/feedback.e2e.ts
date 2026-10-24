import type { Page } from "@playwright/test";
import { axeInBothThemes } from "./axe";
import { expect, consolePath, seed, signedIn, test } from "./fixtures";
import { publicPath } from "./public";

const { hashbrown } = seed.projects;
const RUNS = consolePath(hashbrown.slug, "runs");

test.use({ storageState: signedIn(seed.member) });

/**
 * Answers what the console sends to its test Sentry DSN as Sentry would, or
 * blocks it as a browser's blocker does, and keeps every envelope.
 */
const sentry = async (page: Page, answer: "accept" | "block") => {
	const envelopes: string[] = [];
	await page.route("http://127.0.0.1:9/**", (route) => {
		envelopes.push(route.request().postData() ?? "");
		return answer === "accept"
			? route.fulfill({ status: 200, json: {} })
			: route.abort("blockedbyclient");
	});
	return envelopes;
};

const dialog = (page: Page) =>
	page.getByRole("dialog", { name: "Send feedback" });

const compose = async (page: Page, message: string) => {
	await page.getByRole("button", { name: "Feedback", exact: true }).click();
	await dialog(page).getByRole("textbox", { name: "Message" }).fill(message);
};

// Kills a control that opens nothing, a report that never reaches Sentry,
// one that sends an unticked item or leaves out a ticked one, and a Sent that
// never closes the dialog.
test("Feedback sends a report to Sentry, then closes itself", async ({
	page,
}) => {
	const envelopes = await sentry(page, "accept");
	await page.goto(RUNS);
	await compose(page, "Queued forever");
	await dialog(page)
		.getByRole("checkbox", { name: /Your email/ })
		.uncheck();
	await dialog(page).getByRole("button", { name: "Send" }).click();

	await expect(dialog(page).getByRole("status")).toHaveText("Sent");
	await expect(dialog(page)).toBeHidden({ timeout: 10_000 });
	const report = envelopes.find(
		(envelope) =>
			envelope.includes('"type":"feedback"') &&
			envelope.includes("Queued forever"),
	);
	expect(report).toContain(`"project":"${hashbrown.slug}"`);
	expect(report).toContain(RUNS);
	expect(report).not.toContain(seed.member.user.email);
});

// Kills a blocked report shown as sent, or one that loses what the reader wrote.
test("a report the browser blocks keeps its text", async ({ page }) => {
	await sentry(page, "block");
	await page.goto(RUNS);
	await compose(page, "Queued forever");
	await dialog(page).getByRole("button", { name: "Send" }).click();

	await expect(dialog(page).getByRole("alert")).toContainText(
		"Not sent: blocked by this browser",
	);
	await expect(
		dialog(page).getByRole("textbox", { name: "Message" }),
	).toHaveValue("Queued forever");
});

test.describe("on a phone", () => {
	test.use({ viewport: { width: 390, height: 844 } });

	// Kills a phone bar whose Feedback opens nothing, and a dialog that is not a
	// sheet on the screen's bottom edge.
	test("Feedback opens a sheet", async ({ page }) => {
		await page.goto(RUNS);
		await page.getByRole("button", { name: "Feedback", exact: true }).click();
		await expect(dialog(page)).toBeVisible();
		const box = await dialog(page).boundingBox();
		expect(box && Math.round(box.y + box.height)).toBe(844);
		expect(box?.width).toBe(390);
	});
});

// Kills a Feedback control drawn on the public plot.
test("the public plot has no Feedback control", async ({ page }) => {
	await page.goto(publicPath(hashbrown.slug));
	await expect(page.getByRole("link", { name: "Bencher, home" })).toBeVisible();
	await expect(
		page.getByRole("button", { name: "Feedback", exact: true }),
	).toHaveCount(0);
});

// Kills a dialog or a state of it that fails axe in either theme.
test("the dialog passes axe in both themes, sent", async ({ page }) => {
	await sentry(page, "accept");
	await axeInBothThemes(
		page,
		RUNS,
		() => expect(dialog(page).getByRole("status")).toBeVisible(),
		async () => {
			await compose(page, "Queued forever");
			await dialog(page).getByRole("button", { name: "Send" }).click();
		},
	);
});

// Kills a dialog or a state of it that fails axe in either theme.
test("the dialog passes axe in both themes, failed", async ({ page }) => {
	await sentry(page, "block");
	await axeInBothThemes(
		page,
		RUNS,
		() => expect(dialog(page).getByRole("alert")).toBeVisible(),
		async () => {
			await compose(page, "Queued forever");
			await dialog(page).getByRole("button", { name: "Send" }).click();
		},
	);
});
