import type { Page } from "@playwright/test";
import { expect } from "./fixtures";
import { mailMark, mailedSignIn } from "./mail";

export const digit = (page: Page, n: number) =>
	page.getByRole("textbox", { name: `Digit ${n}` });

/** Type `code` from the first box, as a reader does. */
export const typeCode = async (page: Page, code: string) => {
	await digit(page, 1).focus();
	await page.keyboard.type(code);
};

/** Paste `code` into the box `into`, as a reader's paste from the email does. */
export const pasteCode = (page: Page, code: string, into = 1) =>
	digit(page, into).evaluate((box, code) => {
		const data = new DataTransfer();
		data.setData("text/plain", code);
		box.dispatchEvent(
			new ClipboardEvent("paste", {
				clipboardData: data,
				bubbles: true,
				cancelable: true,
			}),
		);
	}, code);

/**
 * Ask the door on `page` to sign in `email`, as far as Check your inbox, and
 * read the code and the link the API emailed.
 */
export const continueWithEmail = async (page: Page, email: string) => {
	await page.waitForLoadState();
	await page.getByRole("textbox", { name: "Email" }).fill(email);
	const mark = mailMark();
	await page.getByRole("button", { name: "Continue with email" }).click();
	await expect(
		page.getByRole("heading", { level: 1, name: "Check your inbox" }),
	).toBeVisible();
	await expect(page).toHaveURL("/auth/confirm");
	return mailedSignIn(email.toLowerCase(), mark);
};
