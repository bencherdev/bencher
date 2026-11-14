import "./page.css";
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import type { JsonEmailChange } from "../../types/bencher";
import { axeInBothThemes } from "../axe/testing";
import EmailChange, { type EmailChangeEffects, PROFILE } from "./EmailChange";
import Frame from "./Frame";
import type { Answer, RefusalCode } from "./request";

const SECRET = "A".repeat(43);
const CHANGE: JsonEmailChange = {
	email: "ada@example.com",
	new_email: "ada@lovelace.dev",
};

const ok = (data: JsonEmailChange): Answer<JsonEmailChange> => ({
	ok: true,
	data,
});
const refused = (
	status: number | undefined,
	code?: RefusalCode,
): Answer<JsonEmailChange> => ({
	ok: false,
	refused: { status, code, retryAfter: undefined, triesLeft: undefined },
});

const effects = (
	read: Answer<JsonEmailChange>,
	confirm: Answer<JsonEmailChange> = ok(CHANGE),
): EmailChangeEffects => ({
	read: vi.fn(async () => read),
	confirm: vi.fn(async () => confirm),
	leave: vi.fn(),
});

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const mount = (
	fx: EmailChangeEffects,
	{ secret }: { secret: string | undefined } = { secret: SECRET },
) => {
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<Frame>
				<EmailChange secret={secret} effects={fx} />
			</Frame>
		),
		root,
	);
};

const heading = () => page.getByRole("heading", { level: 1 });
const press = () => page.getByRole("button", { name: "Change email" });
const openProfile = () => page.getByRole("link", { name: "Open Profile" });
const terms = () =>
	[...document.querySelectorAll(".kv")].map((kv) => [
		kv.querySelector("dt")?.textContent,
		kv.querySelector("dd")?.textContent,
	]);

// Kills a page that confirms on load, names either address wrong, or leaves
// anywhere but Profile after the press, which it sends once with the secret.
test("names both addresses, and the press confirms once and goes to Profile", async () => {
	const fx = effects(ok(CHANGE));
	mount(fx);

	await expect.element(press()).toBeVisible();
	await expect.element(heading()).toHaveTextContent("Change email");
	expect(terms()).toEqual([
		["from", CHANGE.email],
		["to", CHANGE.new_email],
	]);
	expect(fx.read).toHaveBeenCalledWith(SECRET);
	expect(fx.confirm).not.toHaveBeenCalled();
	expect(page.getByRole("alert").query()).toBeNull();

	await press().click();
	await expect.poll(() => vi.mocked(fx.leave).mock.calls.length).toBe(1);
	expect(fx.confirm).toHaveBeenCalledTimes(1);
	expect(fx.confirm).toHaveBeenCalledWith(SECRET);
});

// Kills a press left live while its confirm is on the way, so a second
// press sends the secret again.
test("the press holds while its confirm is on the way", async () => {
	const fx = effects(ok(CHANGE));
	let settle: (answer: Answer<JsonEmailChange>) => void = () => {};
	vi.mocked(fx.confirm).mockImplementation(
		() =>
			new Promise((resolve) => {
				settle = resolve;
			}),
	);
	mount(fx);

	await press().click();
	await expect.element(press()).toBeDisabled();
	(press().element() as HTMLButtonElement).click();
	expect(fx.confirm).toHaveBeenCalledTimes(1);
	settle(ok(CHANGE));
	await expect.poll(() => vi.mocked(fx.leave).mock.calls.length).toBe(1);
});

// Kills a missing or malformed secret sent to the API, and either read as
// anything but expired.
test("with no secret, or a malformed one, reads as expired and asks nothing", async () => {
	for (const secret of [
		undefined,
		"",
		"nope",
		`${SECRET}A`,
		`${"A".repeat(42)}=`,
	]) {
		const fx = effects(ok(CHANGE));
		mount(fx, { secret });
		await expect.element(page.getByRole("alert")).toHaveTextContent("Expired");
		expect(fx.read).not.toHaveBeenCalled();
		await expect.element(openProfile()).toHaveAttribute("href", PROFILE);
		expect(press().query()).toBeNull();
		dispose?.();
		dispose = undefined;
	}
});

// Kills each of board 05's refusals read wrong, an address shown where the
// board shows none, and a refused link that still offers the press.
test("a link that no longer works says why, with Open Profile", async () => {
	for (const [read, banner] of [
		[refused(400, "email_change_expired"), "Expired"],
		[refused(400, "email_change_used"), "Already used"],
	] as const) {
		mount(effects(read));
		await expect.element(page.getByRole("alert")).toHaveTextContent(banner);
		await expect.element(heading()).toHaveTextContent("Change email");
		await expect.element(openProfile()).toHaveAttribute("href", PROFILE);
		expect(press().query()).toBeNull();
		expect(terms()).toEqual([]);
		dispose?.();
		dispose = undefined;
	}
});

// Kills a press refused as taken, used, expired, or locked that keeps the
// press or names the wrong state, and a taken address the page does not name.
test("a refused press says why, and names a taken address", async () => {
	for (const [confirm, banner, shown] of [
		[refused(409), "Address taken", [["to", CHANGE.new_email]]],
		[refused(400, "email_change_used"), "Already used", []],
		[refused(400, "email_change_expired"), "Expired", []],
		[refused(403, "locked"), "This account is locked", []],
	] as const) {
		const fx = effects(ok(CHANGE), confirm);
		mount(fx);
		await press().click();
		await expect.element(page.getByRole("alert")).toHaveTextContent(banner);
		expect(terms()).toEqual(shown);
		await expect.element(openProfile()).toHaveAttribute("href", PROFILE);
		expect(press().query()).toBeNull();
		expect(fx.leave).not.toHaveBeenCalled();
		dispose?.();
		dispose = undefined;
	}
});

// Kills a failed press that loses the change or the press, and a failed read
// that offers no way to read again.
test("a press or a read that fails can be tried again", async () => {
	const fx = effects(ok(CHANGE), refused(undefined));
	mount(fx);
	await press().click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Not changed");
	await expect.element(press()).toBeEnabled();
	expect(terms()).toEqual([
		["from", CHANGE.email],
		["to", CHANGE.new_email],
	]);
	dispose?.();
	dispose = undefined;

	const unread = effects(refused(500));
	mount(unread);
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Did not load");
	vi.mocked(unread.read).mockResolvedValue(ok(CHANGE));
	await page.getByRole("button", { name: "Try again" }).click();
	await expect.element(press()).toBeVisible();
	expect(unread.read).toHaveBeenCalledTimes(2);
});

// Kills a state of board 05 that fails axe in either theme.
test("each state passes axe in both themes", async () => {
	for (const [read, confirm] of [
		[ok(CHANGE), undefined],
		[refused(400, "email_change_expired"), undefined],
		[refused(400, "email_change_used"), undefined],
		[ok(CHANGE), refused(409)],
	] as const) {
		await axeInBothThemes(async () => {
			mount(effects(read, confirm ?? ok(CHANGE)));
			if (confirm) {
				await press().click();
				await expect.element(page.getByRole("alert")).toBeVisible();
			} else {
				await expect.element(heading()).toBeVisible();
				await expect
					.poll(() => press().query() ?? page.getByRole("alert").query())
					.not.toBeNull();
			}
			return () => {
				dispose?.();
				dispose = undefined;
				document.body.replaceChildren();
			};
		});
	}
});
