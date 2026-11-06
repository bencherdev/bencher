import "./page.css";
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import { VERIFICATION_FAILED } from "./copy";
import Door, { type DoorProps } from "./Door";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const mount = (props: Partial<DoorProps> = {}) => {
	const root = document.createElement("main");
	document.body.append(root);
	dispose = render(
		() => <Door door="login" github={false} google={false} {...props} />,
		root,
	);
};

const email = () => page.getByRole("textbox", { name: "Email" });

// Kills a door that draws a provider the server lacks, or hides one it has.
for (const [github, google] of [
	[true, true],
	[true, false],
	[false, true],
	[false, false],
] as const) {
	test(`draws ${github ? "GitHub" : "no GitHub"} and ${google ? "Google" : "no Google"}`, async () => {
		const onProvider = vi.fn();
		mount({ github, google, onProvider });
		const buttons = [
			["Continue with GitHub", github],
			["Continue with Google", google],
		] as const;
		for (const [name, drawn] of buttons) {
			const button = page.getByRole("button", { name });
			if (drawn) {
				await button.click();
			} else {
				await expect.element(button).not.toBeInTheDocument();
			}
		}
		expect(onProvider.mock.calls).toEqual(
			[github && ["github"], google && ["google"]].filter(Boolean),
		);
		// The divider stands between the providers and the email, so only with a provider.
		expect(document.querySelector(".au-or") !== null).toBe(github || google);
	});
}

// Kills a door that names the wrong page, drops the terms line or a link of
// it, or sends the email with its spaces.
test("each door names itself and carries the terms", async () => {
	const onEmail = vi.fn();
	mount({ door: "signup", onEmail });
	await expect
		.element(
			page.getByRole("heading", { level: 1, name: "Sign up for Bencher" }),
		)
		.toBeVisible();
	for (const [name, href] of [
		["Terms of Use", "/legal/terms-of-use"],
		["Privacy Policy", "/legal/privacy"],
		["License Agreement", "/legal/license"],
	]) {
		await expect
			.element(page.getByRole("link", { name }))
			.toHaveAttribute("href", href);
	}
	await email().fill("  ada@example.com ");
	await page.getByRole("button", { name: "Continue with email" }).click();
	expect(onEmail).toHaveBeenCalledExactlyOnceWith("ada@example.com");
});

// Kills an invite whose address is not filled in, and an address taken back
// from Check your inbox that is not selected to type over.
test("an address is filled in, and selected when taken back", async () => {
	mount({ email: "ada@example.com" });
	await expect.element(email()).toHaveValue("ada@example.com");
	await expect.element(email()).not.toHaveFocus();
	dispose?.();
	document.body.replaceChildren();
	mount({ email: "ada@example.com", select: true });
	await expect.element(email()).toHaveFocus();
	await userEvent.keyboard("bo@example.com");
	await expect.element(email()).toHaveValue("bo@example.com");
});

// Kills a failure with no Try again, or a Try again that does nothing.
test("a failure is the error banner, with Try again", async () => {
	const onRetry = vi.fn();
	mount({ failure: VERIFICATION_FAILED, onRetry });
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Verification failed");
	await page.getByRole("button", { name: "Try again" }).click();
	expect(onRetry).toHaveBeenCalledOnce();
});

// Kills a door that takes a second press while the first is on its way.
test("a busy door takes no press", async () => {
	mount({ github: true, busy: true });
	await expect
		.element(page.getByRole("button", { name: "Continue with email" }))
		.toBeDisabled();
	await expect
		.element(page.getByRole("button", { name: "Continue with GitHub" }))
		.toBeDisabled();
});
