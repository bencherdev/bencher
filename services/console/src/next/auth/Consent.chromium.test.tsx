import "./page.css";
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { Credential, Refusal } from "../../components/auth/authorize/consent";
import {
	type JsonOAuthAuthorization,
	type JsonSession,
	OAuthScope,
	SessionMethod,
} from "../../types/bencher";
import Consent, {
	type Answer,
	type ConsentEffects,
	consentEffects,
} from "./Consent";

const UUID = "0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f";
const EMAIL = "ada@example.com";
const TO = "https://client.example/callback?code=bencher_code_x";

const AUTHORIZATION: JsonOAuthAuthorization = {
	uuid: UUID,
	client: {
		id: "https://client.example/oauth/client.json",
		host: "client.example",
		name: "ChatGPT",
		verified: true,
	},
	redirect: { host: "client.example", loopback: false },
	scopes: [OAuthScope.ProjectsRead],
	expiration: "2026-11-06T00:10:00Z",
};

const SESSION: JsonSession = {
	user: {
		uuid: "00000000-0000-4000-8000-000000000001",
		name: "Ada",
		slug: "ada",
		email: EMAIL,
		admin: false,
		locked: false,
	},
	method: SessionMethod.Email,
	created: "2026-11-06T00:00:00Z",
	expiration: "2027-02-04T00:00:00Z",
};

const effects = (
	session: JsonSession | undefined,
	decide: ConsentEffects["decide"] = async () => ({ to: TO }),
): ConsentEffects => ({
	view: async () => ({ authorization: AUTHORIZATION }),
	session: async () => session,
	decide: vi.fn(decide),
	signOut: vi.fn(async () => {}),
	leave: vi.fn(),
});

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const mount = (fx: ConsentEffects) => {
	const root = document.createElement("main");
	document.body.append(root);
	dispose = render(() => <Consent uuid={UUID} effects={fx} />, root);
};

const button = (name: string) =>
	page.getByRole("button", { name, exact: true });

// Kills a signed in reader asked for a key, an approval sent with no session,
// or one that never leaves for the app.
test("signed in, Approve answers with the session and returns to the app", async () => {
	const fx = effects(SESSION);
	mount(fx);
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("Connect client.example to Bencher");
	await expect.element(page.getByText(EMAIL)).toBeVisible();
	await expect
		.element(page.getByLabelText("Or approve with a user API key"))
		.not.toBeInTheDocument();
	await button("Approve").click();
	expect(fx.decide).toHaveBeenCalledExactlyOnceWith(
		UUID,
		{ approve: true },
		Credential.Session,
	);
	expect(fx.leave).toHaveBeenCalledExactlyOnceWith(TO);
	await expect
		.element(page.getByRole("status"))
		.toHaveTextContent("Returning you to client.example");
});

// Kills a signed out reader offered Approve with no credential, a key sent
// anywhere but the decision, or a key sent with its spaces.
test("signed out, a pasted key approves in the decision", async () => {
	const fx = effects(undefined);
	mount(fx);
	await expect
		.element(
			page.getByRole("heading", { level: 2, name: "Sign in to approve" }),
		)
		.toBeVisible();
	await expect.element(button("Approve")).not.toBeInTheDocument();
	await expect
		.element(page.getByRole("link", { name: "Sign in" }))
		.toHaveAttribute("href", `/auth/login?authorization=${UUID}`);
	await expect
		.element(page.getByRole("link", { name: "Sign up" }))
		.toHaveAttribute("href", `/auth/signup?authorization=${UUID}`);
	const key = `bencher_user_${"a".repeat(30)}`;
	await page.getByLabelText("Or approve with a user API key").fill(` ${key} `);
	await button("Approve with key").click();
	expect(fx.decide).toHaveBeenCalledExactlyOnceWith(
		UUID,
		{ approve: true, key },
		Credential.Key,
	);
});

// Kills a Deny that needs a credential, or sends one.
for (const [who, session] of [
	["signed in", SESSION],
	["signed out", undefined],
] as const) {
	test(`${who}, Deny answers with no credential`, async () => {
		const fx = effects(session);
		mount(fx);
		await button("Deny").click();
		expect(fx.decide).toHaveBeenCalledExactlyOnceWith(
			UUID,
			{ approve: false },
			undefined,
		);
		expect(fx.leave).toHaveBeenCalledExactlyOnceWith(TO);
	});
}

// Kills a session refused at Approve left signed in, or shown as a raw error.
test("a session that ended at Approve asks to sign in again", async () => {
	mount(
		effects(
			SESSION,
			async (): Promise<Answer> => ({ refusal: Refusal.SessionEnded }),
		),
	);
	await button("Approve").click();
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent(
			"Your session ended. Sign in again, or approve with a user API key.",
		);
	await expect
		.element(
			page.getByRole("heading", { level: 2, name: "Sign in to approve" }),
		)
		.toHaveFocus();
	await expect.element(page.getByText(EMAIL)).not.toBeInTheDocument();
});

// Kills Use a different account that keeps the session, or the reader shown.
test("Use a different account signs out and asks to sign in", async () => {
	const fx = effects(SESSION);
	mount(fx);
	await button("Use a different account").click();
	expect(fx.signOut).toHaveBeenCalledOnce();
	await expect
		.element(
			page.getByRole("heading", { level: 2, name: "Sign in to approve" }),
		)
		.toHaveFocus();
	await expect.element(page.getByText(EMAIL)).not.toBeInTheDocument();
});

// Kills a page that keeps the hint of a session the API answers 401 for, as
// it reads the session or approves with it, and one that forgets the reader
// when only a pasted key was refused.
test("a 401 for the session forgets the reader, and one for a key does not", async () => {
	const answers: Record<string, number> = {};
	const fetch = vi.fn(async (input: RequestInfo | URL) => {
		const status = answers[String(input)] ?? 200;
		return new Response("{}", { status });
	});
	const forget = vi.fn();
	const effects = consentEffects(async () => {}, forget, fetch);
	const decision = `/api/v0/oauth/authorizations/${UUID}`;

	answers["/api/v0/auth/session"] = 401;
	expect(await effects.session()).toBeUndefined();
	expect(forget).toHaveBeenCalledOnce();

	answers[decision] = 401;
	expect(
		await effects.decide(UUID, { approve: true }, Credential.Session),
	).toEqual({ refusal: Refusal.SessionEnded });
	expect(forget).toHaveBeenCalledTimes(2);

	expect(
		await effects.decide(UUID, { approve: true, key: "k" }, Credential.Key),
	).toEqual({ refusal: Refusal.KeyRefused });
	expect(forget).toHaveBeenCalledTimes(2);
});
