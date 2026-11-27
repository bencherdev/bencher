import "./page.css";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { JsonSignedIn, JsonSignInEmail } from "../../types/bencher";
import Inbox, { type InboxEffects, type InboxProps } from "./Inbox";
import type { Answer, RefusalCode, Refused } from "./request";

const EMAIL = "ada@example.com";
const SIGNED_IN: JsonSignedIn = {
	user: {
		uuid: "00000000-0000-4000-8000-000000000001",
		name: "Ada",
		slug: "ada",
		email: EMAIL,
		admin: false,
		locked: false,
	},
	new: false,
};

const refused = (
	code: RefusalCode | undefined,
	more: Partial<Refused> = {},
): Answer<never> => ({
	ok: false,
	refused: {
		status: 403,
		code,
		retryAfter: undefined,
		triesLeft: undefined,
		...more,
	},
});

const effects = (overrides: Partial<InboxEffects> = {}): InboxEffects => ({
	prove: vi.fn(async () => ({ ok: true, data: SIGNED_IN }) as const),
	resend: vi.fn(
		async (): Promise<Answer<JsonSignInEmail>> => ({
			ok: true,
			data: { email: EMAIL, resend_after: 60 },
		}),
	),
	another: vi.fn(),
	signedIn: vi.fn(),
	dated: vi.fn(),
	...overrides,
});

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const mount = (props: Partial<InboxProps> & { effects: InboxEffects }) => {
	const root = document.createElement("main");
	document.body.append(root);
	// Spread as an object, since a prop Solid reads as undefined keeps the default.
	const all = { email: EMAIL, ...props };
	dispose = render(() => <Inbox {...all} />, root);
};

const digit = (n: number) => page.getByRole("textbox", { name: `Digit ${n}` });
const boxes = () =>
	[...document.querySelectorAll<HTMLInputElement>(".au-digit")].map(
		(box) => box.value,
	);
const paste = (code: string, into = 1) => {
	const data = new DataTransfer();
	data.setData("text/plain", code);
	digit(into)
		.element()
		.dispatchEvent(
			new ClipboardEvent("paste", {
				clipboardData: data,
				bubbles: true,
				cancelable: true,
			}),
		);
};

// Kills an inbox that hides the address, the code's boxes, Resend, or Use
// another email, or that leaves the first box unfocused.
test("entering shows the address, six boxes, Resend, and Use another email", async () => {
	mount({ effects: effects(), resendAt: 0 });
	await expect
		.element(page.getByRole("heading", { level: 1, name: "Check your inbox" }))
		.toBeVisible();
	await expect.element(page.getByText(EMAIL)).toBeVisible();
	expect(document.querySelectorAll(".au-digit")).toHaveLength(6);
	await expect.element(digit(1)).toHaveFocus();
	await expect
		.element(page.getByRole("button", { name: "Resend", exact: true }))
		.toBeEnabled();
	await expect
		.element(page.getByRole("button", { name: "Use another email" }))
		.toBeEnabled();
});

// Kills a provider's sign-in that hides which account asks, draws it below
// the heading, or under another provider's mark; and a sign-in by email that
// draws an account at all.
test("a provider's account to prove sits above the heading with its mark", async () => {
	for (const [provider, mark] of [
		["github", "svg.mark"],
		["google", 'img.mark[src="/google.svg"]'],
	] as const) {
		mount({ effects: effects(), who: { provider, handle: "ada-work" } });
		const card = document.querySelector("main") as HTMLElement;
		const who = card.querySelector(".au-who");
		expect(who?.textContent, provider).toBe("ada-work");
		expect(who?.querySelector(mark), provider).not.toBeNull();
		expect(card.firstElementChild, provider).toBe(who);
		expect(who?.nextElementSibling?.textContent, provider).toBe(
			"Check your inbox",
		);
		dispose?.();
		document.body.replaceChildren();
	}
	mount({ effects: effects() });
	expect(document.querySelector(".au-who")).toBeNull();
});

// Kills a paste that fills only the box it lands in, keeps the non-digits a
// copied code carries, or waits for a press to send the code.
test("a pasted code fills every box and sends", async () => {
	const fx = effects();
	mount({ effects: fx });
	paste("12 34-56", 3);
	expect(boxes()).toEqual(["1", "2", "3", "4", "5", "6"]);
	expect(fx.prove).toHaveBeenCalledExactlyOnceWith("123456");
	await expect
		.poll(() => vi.mocked(fx.signedIn).mock.calls)
		.toEqual([[SIGNED_IN]]);
});

// Kills typing that stays in one box, a Backspace that leaves the box before
// it alone, and a code sent before its sixth digit.
test("typed digits move on, Backspace goes back, and the sixth digit sends", async () => {
	const fx = effects();
	mount({ effects: fx });
	await userEvent.keyboard("12");
	await expect.element(digit(3)).toHaveFocus();
	await userEvent.keyboard("{Backspace}");
	expect(boxes().slice(0, 3)).toEqual(["1", "", ""]);
	await expect.element(digit(2)).toHaveFocus();
	await userEvent.keyboard("23456");
	expect(fx.prove).not.toHaveBeenCalledWith("12345");
	expect(fx.prove).toHaveBeenCalledExactlyOnceWith("123456");
});

// Kills a wrong code that locks the boxes while tries are left, keeps the
// focus off them, or never says so; and a fix to one box that spends a try
// before the sixth digit is typed again.
test("a wrong code with tries left says so and keeps the boxes", async () => {
	const fx = effects({
		prove: vi.fn(async () => refused("wrong_code", { triesLeft: 3 })),
	});
	mount({ effects: fx });
	paste("111111");
	await expect.element(page.getByRole("alert")).toHaveTextContent("Wrong code");
	await expect.element(digit(1)).toBeEnabled();
	await expect.element(digit(1)).toHaveAttribute("aria-invalid", "true");
	await expect.element(digit(1)).toHaveFocus();
	await userEvent.keyboard("2");
	await expect.element(page.getByRole("alert")).not.toBeInTheDocument();
	await expect.element(digit(2)).toHaveFocus();
	expect(boxes()).toEqual(["2", "1", "1", "1", "1", "1"]);
	expect(fx.prove).toHaveBeenCalledOnce();
	await digit(6).click();
	await userEvent.keyboard("7");
	expect(fx.prove).toHaveBeenLastCalledWith("211117");
});

// Kills a last wrong code read as one more try, or one with no way on.
for (const [why, answer] of [
	["the last wrong code", refused("wrong_code", { triesLeft: 0 })],
	["no tries left", refused("no_tries_left")],
] as const) {
	test(`${why} greys the boxes and offers Resend first`, async () => {
		const fx = effects({ prove: async () => answer });
		mount({ effects: fx, resendAt: 0 });
		paste("111111");
		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent("No tries left");
		await expect.element(digit(1)).toBeDisabled();
		expect(boxes()).toEqual(["", "", "", "", "", ""]);
		const resend = page.getByRole("button", { name: "Resend", exact: true });
		await expect.element(resend).toHaveAttribute("data-variant", "primary");
		await resend.click();
		expect(fx.resend).toHaveBeenCalledOnce();
		await expect.element(page.getByRole("status")).toHaveTextContent("Sent");
		await expect.element(digit(1)).toBeEnabled();
	});
}

// Kills an ended sign-in shown as a wrong code, or with no way on.
test("an ended sign-in is expired, with Resend first", async () => {
	mount({
		effects: effects({ prove: async () => refused("sign_in_ended") }),
		resendAt: 0,
	});
	paste("111111");
	await expect.element(page.getByRole("alert")).toHaveTextContent("Expired");
	await expect.element(digit(1)).toBeDisabled();
	await expect
		.element(page.getByRole("button", { name: "Resend", exact: true }))
		.toHaveAttribute("data-variant", "primary");
});

// Kills paused codes that leave the boxes open, or suggest another try.
test("paused codes grey the boxes and point to the link", async () => {
	mount({ effects: effects({ prove: async () => refused("codes_paused") }) });
	paste("111111");
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Too many wrong codes. Use the link in your email.");
	for (const n of [1, 2, 3, 4, 5, 6]) {
		await expect.element(digit(n)).toBeDisabled();
	}
	expect(document.body.textContent).not.toMatch(/try again/i);
});

// Kills a Resend that reopens the boxes while the address's codes are paused,
// which invites a code the API refuses, and a pause that outlasts its
// `Retry-After`.
test("codes stay paused through a Resend until the pause ends", async () => {
	const [clock, setClock] = createSignal(1_000_000);
	const fx = effects({
		prove: async () =>
			refused("codes_paused", { status: 429, retryAfter: 600 }),
	});
	mount({ effects: fx, resendAt: 0, now: clock });
	paste("111111");
	const pausedText = "Too many wrong codes. Use the link in your email.";
	await expect.element(page.getByRole("alert")).toHaveTextContent(pausedText);

	await page.getByRole("button", { name: "Resend", exact: true }).click();
	await expect
		.element(page.getByRole("button", { name: "Resend in 60s" }))
		.toBeDisabled();
	expect(fx.resend).toHaveBeenCalledOnce();
	await expect.element(digit(1)).toBeDisabled();
	await expect.element(page.getByRole("alert")).toHaveTextContent(pausedText);

	setClock(1_600_000);
	await expect.element(digit(1)).toBeEnabled({ timeout: 2_000 });
	await expect.element(page.getByRole("alert")).not.toBeInTheDocument();
});

// Kills a Resend that opens before its wait, a countdown that never ends, and
// a `Retry-After` the page ignores.
test("Resend waits its seconds, counting down, and takes a refusal's wait", async () => {
	const [clock, setClock] = createSignal(1_000_000);
	const fx = effects({
		resend: async () => refused("resend_waiting", { retryAfter: 42 }),
	});
	mount({ effects: fx, resendAt: 1_030_000, now: clock });
	const waiting = page.getByRole("button", { name: "Resend in 30s" });
	await expect.element(waiting).toBeDisabled();
	setClock(1_030_000);
	const open = page.getByRole("button", { name: "Resend", exact: true });
	await expect.element(open).toBeEnabled({ timeout: 2_000 });
	await open.click();
	await expect
		.element(page.getByRole("button", { name: "Resend in 42s" }))
		.toBeDisabled();
	expect(fx.dated).toHaveBeenLastCalledWith({ resendAt: 1_072_000 });
});

// Kills an address at its cap shown as sent, without its real wait, or with
// Resend open before the wait ends.
test("an address at its cap says how long to wait", async () => {
	const fx = effects({
		resend: async () => ({ ok: true, data: { email: EMAIL, wait: 3_000 } }),
	});
	mount({ effects: fx, resendAt: 0, now: () => 5_000_000 });
	await page.getByRole("button", { name: "Resend", exact: true }).click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent(
			"Too many emails to this address. Try again in 50 minutes.",
		);
	await expect
		.element(page.getByRole("button", { name: "Resend", exact: true }))
		.toBeDisabled();
	expect(fx.dated).toHaveBeenLastCalledWith({ cappedUntil: 8_000_000 });
});

// Kills a reload at the cap that forgets its wait.
test("a cap kept from before the reload still waits", async () => {
	mount({ effects: effects(), cappedUntil: 9_000, now: () => 1_000 });
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent(
			"Too many emails to this address. Try again in 1 minute.",
		);
});

// Kills a refusal this page does not know read as one it does, such as a
// 500's issue ID, and a Try again that tries nothing.
test("an unknown refusal is the generic error, and Try again sends again", async () => {
	let tries = 0;
	const fx = effects({
		prove: vi.fn(async () => {
			tries += 1;
			return tries === 1
				? refused(undefined, { status: 500 })
				: ({ ok: true, data: SIGNED_IN } as const);
		}),
	});
	mount({ effects: fx });
	paste("123456");
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Sign-in failed");
	await page.getByRole("button", { name: "Try again" }).click();
	expect(fx.prove).toHaveBeenLastCalledWith("123456");
	await expect.poll(() => vi.mocked(fx.signedIn).mock.calls).toHaveLength(1);
});

// Kills a locked account shown as a generic failure, or offered a retry.
test("a locked account says so, with no Try again", async () => {
	mount({ effects: effects({ prove: async () => refused("locked") }) });
	paste("123456");
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("This account is locked");
	await expect
		.element(page.getByRole("button", { name: "Try again" }))
		.not.toBeInTheDocument();
});

// Kills a Use another email that does nothing, and an old link's page,
// which knows no address, offering Resend.
test("Use another email leaves, and with no address there is no Resend", async () => {
	const fx = effects();
	mount({ effects: fx, email: undefined, status: { kind: "expired" } });
	await expect.element(page.getByRole("alert")).toHaveTextContent("Expired");
	await expect
		.element(page.getByRole("button", { name: /^Resend/ }))
		.not.toBeInTheDocument();
	await page.getByRole("button", { name: "Use another email" }).click();
	expect(fx.another).toHaveBeenCalledOnce();
});
