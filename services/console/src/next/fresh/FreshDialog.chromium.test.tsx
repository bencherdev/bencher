import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../auth/code.css";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonFreshSignIn,
	JsonProved,
	JsonSignInEmail,
	SignInProvider,
} from "../../types/bencher";
import type { Failure } from "../auth/copy";
import type { Answer, RefusalCode, Refused } from "../auth/request";
import { signInAgain } from "./dialog";
import FreshDialog, { type FreshEffects } from "./FreshDialog";

const EMAIL = "ada@example.com";
const PROVED: JsonProved = { proved: "2026-11-07T12:00:00Z" };

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

const pinned = (...providers: string[]): Answer<JsonFreshSignIn> => ({
	ok: true,
	data: { providers: providers as SignInProvider[] },
});

const emailed: Answer<JsonSignInEmail> = {
	ok: true,
	data: { email: EMAIL, resend_after: 60 },
};

/** The effects a test stands in for, and the listener a proof in another tab would call. */
const effects = (overrides: Partial<FreshEffects> = {}) => {
	const heard: (() => void)[] = [];
	const fx: FreshEffects = {
		methods: vi.fn(async () => pinned("github")),
		email: vi.fn(async () => emailed),
		resend: vi.fn(async () => emailed),
		prove: vi.fn(async () => ({ ok: true, data: PROVED }) as const),
		provider: vi.fn(async (): Promise<Failure | undefined> => undefined),
		onProved: vi.fn((listener: () => void) => {
			heard.push(listener);
			return () => {
				heard.splice(heard.indexOf(listener), 1);
			};
		}),
		...overrides,
	};
	return { fx, heard };
};

let dispose: (() => void) | undefined;
beforeEach(async () => {
	await page.viewport(1280, 720);
});
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
	vi.unstubAllGlobals();
	vi.restoreAllMocks();
});

const mount = (fx: FreshEffects) => {
	const done = vi.fn();
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(() => <FreshDialog effects={fx} onDone={done} />, root);
	return done;
};

const dialog = () => page.getByRole("dialog", { name: "Sign in again" });
const digit = (n: number) => page.getByRole("textbox", { name: `Digit ${n}` });

// Kills a dialog that offers a provider the reader has no pin for, leaves
// out one they have, or offers no email.
test("offers each provider pinned to the reader, then Email a code", async () => {
	const { fx } = effects({
		methods: vi.fn(async () => pinned("google", "github")),
	});
	mount(fx);
	await expect.element(dialog()).toBeVisible();
	await expect
		.poll(() =>
			[...document.querySelectorAll(".au-stack .ui-button")].map(
				(button) => button.textContent,
			),
		)
		.toEqual(["Continue with Google", "Continue with GitHub", "Email a code"]);
	dispose?.();
	document.body.replaceChildren();

	const failed = effects({ methods: vi.fn(async () => refused(undefined)) });
	mount(failed.fx);
	await expect
		.element(page.getByRole("button", { name: "Email a code" }))
		.toBeVisible();
	await expect
		.element(page.getByRole("button", { name: /Continue with/ }))
		.not.toBeInTheDocument();
});

// Kills an email step that loses the address, the six boxes, the Resend
// countdown, or Back, and a right code that leaves the dialog open.
test("Email a code goes to the code, whose right code ends it proved", async () => {
	const { fx } = effects();
	const done = mount(fx);
	await page.getByRole("button", { name: "Email a code" }).click();
	expect(fx.email).toHaveBeenCalledOnce();
	await expect.element(dialog().getByText(EMAIL)).toBeVisible();
	expect(document.querySelectorAll(".au-digit")).toHaveLength(6);
	await expect.element(digit(1)).toHaveFocus();
	await expect
		.element(page.getByRole("button", { name: /^Resend in \d+s$/ }))
		.toBeDisabled();
	await expect
		.element(page.getByRole("button", { name: "Back" }))
		.toBeVisible();
	await userEvent.keyboard("123456");
	expect(fx.prove).toHaveBeenCalledExactlyOnceWith("123456");
	await expect.poll(() => done.mock.calls).toEqual([[true]]);
});

// Kills a wrong code that ends the dialog, and one whose refusal it hides.
test("a wrong code says so and keeps the dialog open", async () => {
	const { fx } = effects({
		prove: vi.fn(async () => refused("wrong_code", { triesLeft: 4 })),
	});
	const done = mount(fx);
	await page.getByRole("button", { name: "Email a code" }).click();
	await userEvent.keyboard("111111");
	await expect.element(page.getByRole("alert")).toHaveTextContent("Wrong code");
	expect(done).not.toHaveBeenCalled();
});

// Kills paused codes that do not point the reader to the link, which still
// proves the session.
test("paused codes point to the link", async () => {
	const { fx } = effects({
		prove: vi.fn(async () => refused("codes_paused", { retryAfter: 600 })),
	});
	mount(fx);
	await page.getByRole("button", { name: "Email a code" }).click();
	await userEvent.keyboard("111111");
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Too many wrong codes. Use the link in your email.");
	await expect.element(digit(1)).toBeDisabled();
});

// Kills a Resend of a fresh sign-in that ended which leaves the reader with
// no code, rather than starting one again.
test("Resend after the sign-in ended starts another", async () => {
	// An email whose Resend is open at once.
	const { fx } = effects({
		email: vi.fn(async () => ({ ok: true, data: { email: EMAIL } }) as const),
		prove: vi.fn(async () => refused("no_tries_left")),
		resend: vi.fn(async () => refused("sign_in_ended", { status: 401 })),
	});
	mount(fx);
	await page.getByRole("button", { name: "Email a code" }).click();
	await userEvent.keyboard("111111");
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("No tries left");
	vi.mocked(fx.email).mockClear();
	await page.getByRole("button", { name: "Resend", exact: true }).click();
	expect(fx.resend).toHaveBeenCalledOnce();
	await expect.poll(() => vi.mocked(fx.email).mock.calls.length).toBe(1);
	await expect.element(page.getByRole("status")).toHaveTextContent("Sent");
});

// Kills Back that leaves the reader on the code, and a start's refusal that
// says nothing or cannot be tried again.
test("Back returns to the ways in, and a refused email can be tried again", async () => {
	const { fx } = effects({
		email: vi
			.fn()
			.mockResolvedValueOnce(emailed)
			.mockResolvedValueOnce(refused("email_failed", { status: 503 }))
			.mockResolvedValueOnce(emailed),
	});
	mount(fx);
	await page.getByRole("button", { name: "Email a code" }).click();
	await page.getByRole("button", { name: "Back" }).click();
	const email = page.getByRole("button", { name: "Email a code" });
	await email.click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("The email did not send");
	await page.getByRole("button", { name: "Try again" }).click();
	await expect.element(dialog().getByText(EMAIL)).toBeVisible();
	expect(fx.email).toHaveBeenCalledTimes(3);
});

// Kills a provider's press that starts nothing, starts another provider, or
// hides why it failed.
test("a provider's press starts it, and a failure can be tried again", async () => {
	const { fx } = effects({
		provider: vi
			.fn()
			.mockResolvedValueOnce({ text: "Sign-in failed", retry: true })
			.mockResolvedValueOnce(undefined),
	});
	const done = mount(fx);
	await page.getByRole("button", { name: "Continue with GitHub" }).click();
	expect(fx.provider).toHaveBeenCalledExactlyOnceWith("github");
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Sign-in failed");
	await page.getByRole("button", { name: "Try again" }).click();
	expect(fx.provider).toHaveBeenCalledTimes(2);
	await expect.element(page.getByRole("alert")).not.toBeInTheDocument();
	expect(done).not.toHaveBeenCalled();
});

// Kills a dialog that waits on, rather than hears, a proof by a link or a
// provider's window in another tab, and one that keeps listening once gone.
test("a proof in another tab ends it proved", async () => {
	const { fx, heard } = effects();
	const done = mount(fx);
	await expect.element(dialog()).toBeVisible();
	expect(heard).toHaveLength(1);
	heard[0]?.();
	expect(done.mock.calls).toEqual([[true]]);
	dispose?.();
	dispose = undefined;
	expect(heard).toHaveLength(0);
});

// Kills a Cancel, a Close, or an Escape that reads as a proof, or does nothing.
test("Cancel, Close, and Escape end it unproved", async () => {
	for (const leave of ["Cancel", "Close", "Escape"]) {
		const { fx } = effects();
		const done = mount(fx);
		await expect.element(dialog()).toBeVisible();
		if (leave === "Escape") {
			await userEvent.keyboard("{Escape}");
		} else {
			await dialog().getByRole("button", { name: leave }).click();
		}
		expect(done.mock.calls, leave).toEqual([[false]]);
		dispose?.();
		document.body.replaceChildren();
	}
});

/** The console's relay as these tests answer it, and every request it was sent. */
const relay = (answers: Record<string, unknown>) => {
	const sent: { url: string; init: RequestInit | undefined }[] = [];
	vi.stubGlobal(
		"fetch",
		vi.fn(async (url: string, init?: RequestInit) => {
			sent.push({ url, init });
			const key = `${init?.method ?? "GET"} ${url}`;
			return new Response(JSON.stringify(answers[key] ?? {}), {
				status: key in answers ? 200 : 500,
			});
		}),
	);
	return sent;
};

// Kills a dialog that asks anywhere but the fresh sign-in's own endpoints,
// stays on the page once proved, resolves unproved, or proves by code without
// telling the other tabs.
test("signInAgain asks the API by email and resolves proved", async () => {
	const written = vi.spyOn(Storage.prototype, "setItem");
	const sent = relay({
		"GET /api/v0/auth/fresh": { providers: [] },
		"POST /api/v0/auth/fresh/email": { email: EMAIL, resend_after: 60 },
		"POST /api/v0/auth/fresh/code": PROVED,
	});
	const proved = signInAgain();
	await page.getByRole("button", { name: "Email a code" }).click();
	await expect.element(digit(1)).toHaveFocus();
	await userEvent.keyboard("123456");
	expect(await proved).toBe(true);
	expect(document.querySelector("dialog")).toBeNull();
	expect(sent.map(({ url, init }) => [init?.method ?? "GET", url])).toEqual([
		["GET", "/api/v0/auth/fresh"],
		["POST", "/api/v0/auth/fresh/email"],
		["POST", "/api/v0/auth/fresh/code"],
	]);
	expect(sent[2]?.init?.body).toBe('{"code":"123456"}');
	expect(written).toHaveBeenCalledWith("BENCHER_PROVED", "1");
});

// Kills changes refused together that each open a dialog of their own, and an
// ended dialog that answers the next change that needs one.
test("changes that need a fresh sign-in together share one dialog", async () => {
	relay({ "GET /api/v0/auth/fresh": { providers: [] } });
	const first = signInAgain();
	const second = signInAgain();
	await expect.element(dialog()).toBeVisible();
	expect(document.querySelectorAll("dialog")).toHaveLength(1);
	await dialog().getByRole("button", { name: "Cancel" }).click();
	expect([await first, await second]).toEqual([false, false]);
	const third = signInAgain();
	await expect.element(dialog()).toBeVisible();
	window.dispatchEvent(
		new StorageEvent("storage", { key: "BENCHER_PROVED", newValue: "1" }),
	);
	expect(await third).toBe(true);
});

// Kills a provider that loads in this tab, in a window opened only after the
// start answers (which a browser blocks), with no mark that it signs in
// again, or at a URL other than the start's.
test("a provider signs in again in a window of its own", async () => {
	const url = "https://github.example/login/oauth/authorize?state=s";
	relay({
		"GET /api/v0/auth/fresh": { providers: ["github"] },
		"POST /api/v0/auth/fresh/providers/github/start": { url },
	});
	const kept = new Map<string, string>();
	const popup = {
		sessionStorage: {
			setItem: (key: string, value: string) => kept.set(key, value),
			getItem: (key: string) => kept.get(key) ?? null,
			removeItem: (key: string) => kept.delete(key),
		},
		location: { replace: vi.fn() },
		close: vi.fn(),
	};
	let opened = 0;
	vi.stubGlobal(
		"open",
		vi.fn(() => {
			opened = vi.mocked(fetch).mock.calls.length;
			return popup;
		}),
	);
	const proved = signInAgain();
	await page.getByRole("button", { name: "Continue with GitHub" }).click();
	await expect.poll(() => popup.location.replace.mock.calls).toEqual([[url]]);
	expect(opened).toBe(1);
	expect([...kept]).toEqual([["BENCHER_FRESH_WINDOW", "1"]]);
	expect(popup.close).not.toHaveBeenCalled();
	window.dispatchEvent(
		new StorageEvent("storage", { key: "BENCHER_PROVED", newValue: "1" }),
	);
	expect(await proved).toBe(true);
});
