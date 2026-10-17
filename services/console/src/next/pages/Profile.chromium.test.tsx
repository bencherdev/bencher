import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, describe, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { JsonAuthUser, JsonUser } from "../../types/bencher";
import { AccountContext } from "../account/context";
import { type Api, ApiError, type Method } from "../api";
import Profile from "./Profile";

const USER: JsonUser = {
	uuid: "user-uuid",
	name: "Muriel Bagge",
	slug: "muriel-bagge",
	email: "muriel@nowhere.example",
	admin: false,
	locked: false,
};

const READER = { user: USER, token: "tok" } as JsonAuthUser;
/** A server admin, who can edit other users. */
const ADMIN = {
	user: {
		...USER,
		uuid: "admin-uuid",
		name: "Admin Bagge",
		slug: "admin-bagge",
		email: "admin@nowhere.example",
	},
	token: "tok",
} as JsonAuthUser;

interface Sent {
	method: Method;
	path: string;
	body: unknown;
}

/** The user API with `tokens` unrevoked legacy tokens, answering each change with `answer`. */
const userApi = (
	tokens: number,
	answer: (sent: Sent) => Promise<JsonUser> = async (sent) => ({
		...USER,
		...(sent.body as object),
	}),
) => {
	const sent: Sent[] = [];
	let user = USER;
	const api: Api = {
		get: async <T,>(path: string) =>
			path.endsWith("/tokens?per_page=1")
				? {
						data: [] as T,
						headers: new Headers({ "X-Total-Count": String(tokens) }),
					}
				: { data: user as T, headers: new Headers() },
		send: async <T,>(method: Method, path: string, body?: unknown) => {
			const change = { method, path, body };
			sent.push(change);
			user = await answer(change);
			return { data: user as T, headers: new Headers() };
		},
	};
	return { api, sent };
};

let dispose: (() => void) | undefined;
/** The users the console kept as the reader's own, and how often it signed out. */
let kept: JsonUser[] = [];
let signedOut = 0;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	kept = [];
	signedOut = 0;
	document.body.replaceChildren();
});

const mount = (
	api: Api,
	{
		reader = READER,
		user = USER.slug,
	}: { reader?: JsonAuthUser; user?: string } = {},
) => {
	const history = createMemoryHistory();
	history.set({ value: `/users/${user}/settings`, replace: true });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	const client = new QueryClient({
		defaultOptions: { queries: { retry: false } },
	});
	const stop = render(
		() => (
			<QueryClientProvider client={client}>
				<AccountContext.Provider
					value={{
						api,
						reader: () => reader,
						keepUser: (user) => kept.push(user),
						signOut: () => {
							signedOut += 1;
						},
					}}
				>
					<MemoryRouter history={history}>
						<Route
							path="/users/:user/settings"
							component={() => (
								<Suspense fallback={<p>Suspended</p>}>
									<Profile />
								</Suspense>
							)}
						/>
					</MemoryRouter>
				</AccountContext.Provider>
			</QueryClientProvider>
		),
		root,
	);
	dispose = () => {
		stop();
		root.remove();
	};
	return { client, history };
};

const textbox = (name: string) => page.getByRole("textbox", { name });
const save = () => page.getByRole("button", { name: "Save changes" });
const legacyLink = () => page.getByRole("link", { name: /legacy token/ });

describe("the form", () => {
	// Kills a save sent anywhere but the user's update with only what changed,
	// a form left dirty after the API accepts, and a Saved that never shows.
	test("saves what changed through the user API and says so", async () => {
		const { api, sent } = userApi(0);
		mount(api);

		await expect.element(save()).toBeDisabled();
		await textbox("Name").fill("Muriel Courage");
		await save().click();

		await expect.element(page.getByRole("status")).toHaveTextContent("Saved");
		expect(sent).toEqual([
			{
				method: "PATCH",
				path: `/v0/users/${USER.uuid}`,
				body: { name: "Muriel Courage" },
			},
		]);
		await expect.element(save()).toBeDisabled();
		await expect.element(textbox("Name")).toHaveValue("Muriel Courage");
	});

	// Kills a Discard that keeps the edit, and a form that paints only after
	// the API answers when the reader's own account is already known.
	test("paints the reader at once, and Discard puts it back", async () => {
		const { api } = userApi(0);
		api.get = () => new Promise(() => {});
		mount(api);

		await expect.element(textbox("Email")).toHaveValue(USER.email);
		await textbox("Slug").fill("muriel");
		await page.getByRole("button", { name: "Discard" }).click();

		await expect.element(textbox("Slug")).toHaveValue(USER.slug);
		await expect.element(save()).toBeDisabled();
	});

	// Kills a slug the API would refuse sent anyway, and a name left empty.
	test("holds Save back while a field cannot be saved", async () => {
		const { api } = userApi(0);
		mount(api);

		await textbox("Slug").fill("Muriel Bagge");
		await expect.element(save()).toBeDisabled();
		await textbox("Slug").fill(USER.slug);
		await textbox("Name").fill(" ");
		await expect.element(save()).toBeDisabled();
	});

	// Kills a page left on the old slug, whose user no longer answers, and a
	// reader the console still knows by it.
	test("a new slug moves the page and the reader to it", async () => {
		const { api } = userApi(0);
		const { history } = mount(api);

		await textbox("Slug").fill("muriel");
		await save().click();

		await expect.poll(() => history.get()).toBe("/users/muriel/settings");
		expect(kept.map(({ slug }) => slug)).toEqual(["muriel"]);
		await expect.element(textbox("Slug")).toHaveValue("muriel");
		await expect.element(save()).toBeDisabled();
	});

	// Kills a refusal said nowhere, and a form that drops the reader's edit when refused.
	test("a refusal says why and keeps the edit", async () => {
		const { api } = userApi(0, async () => {
			throw new ApiError(409, "client", '{"message":"Slug taken"}');
		});
		mount(api);

		await textbox("Slug").fill("muriel");
		await save().click();

		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent("Slug taken");
		await expect.element(textbox("Slug")).toHaveValue("muriel");
		await expect.element(save()).toBeEnabled();
	});
});

describe("what the page knows", () => {
	/** An API whose user reads fail with `error`, and whose count is zero. */
	const failing = (error: ApiError) => {
		const { api } = userApi(0);
		const get = api.get;
		let fail = true;
		api.get = <T,>(path: string, signal?: AbortSignal) =>
			fail && !path.includes("/tokens")
				? Promise.reject(error)
				: get<T>(path, signal);
		return { api, heal: () => (fail = false) };
	};

	// Kills a form that waits for the API when the page is the reader's own,
	// by slug or by uuid, and one that paints the reader for another user.
	test("paints the reader's own account at once, by slug or uuid, and no one else's", async () => {
		const held = userApi(0).api;
		held.get = () => new Promise(() => {});
		mount(held, { user: USER.uuid });
		await expect.element(textbox("Email")).toHaveValue(USER.email);
		dispose?.();

		mount(held, { reader: ADMIN });
		await expect
			.element(page.getByRole("heading", { name: "Profile" }))
			.toBeVisible();
		expect(textbox("Email").query()).toBeNull();
	});

	// Kills a read failure that takes away the form the reader already sees.
	test("a failed read keeps the reader's painted form", async () => {
		const { api } = failing(new ApiError(undefined, "network", "offline"));
		mount(api);

		await expect.element(textbox("Email")).toHaveValue(USER.email);
		await new Promise((resolve) => setTimeout(resolve, 50));
		await expect.element(textbox("Email")).toHaveValue(USER.email);
		expect(page.getByRole("alert").query()).toBeNull();
	});

	// Kills an unknown user left on a skeleton forever, and a Retry that asks nothing.
	test("an unknown user shows the failure, and Retry asks again", async () => {
		const { api, heal } = failing(new ApiError(404, "not_found", "{}"));
		mount(api, { reader: ADMIN });

		const alert = page.getByRole("alert");
		await expect.element(alert).toHaveTextContent("Profile did not load");
		heal();
		await alert.getByRole("button", { name: "Retry" }).click();
		await expect.element(textbox("Email")).toHaveValue(USER.email);
	});

	// Kills a change made elsewhere that never reaches an untouched form, and
	// one that overwrites what the reader is typing.
	test("a change made elsewhere reaches the form only while it is untouched", async () => {
		const { client } = mount(userApi(0).api);
		const key = ["account", "user", USER.slug];

		await expect.element(textbox("Name")).toHaveValue(USER.name);
		client.setQueryData(key, { ...USER, name: "Muriel Courage" });
		await expect.element(textbox("Name")).toHaveValue("Muriel Courage");

		await textbox("Slug").fill("muriel");
		client.setQueryData(key, { ...USER, name: "Courage Bagge" });
		await expect.element(textbox("Slug")).toHaveValue("muriel");
		await expect.element(textbox("Name")).toHaveValue("Muriel Courage");
	});
});

describe("legacy tokens", () => {
	// Kills a link drawn at no tokens, a confirmation asked with nothing to
	// revoke, an email change held back for one, and a session kept after
	// the email it was signed in with is gone.
	test("with none, there is no link and an email change saves and signs out", async () => {
		const { api, sent } = userApi(0);
		mount(api);

		await expect.element(textbox("Email")).toHaveValue(USER.email);
		await textbox("Email").fill("muriel@courage.example");
		await save().click();

		await expect.poll(() => signedOut).toBe(1);
		expect(sent.map(({ body }) => body)).toEqual([
			{ email: "muriel@courage.example" },
		]);
		expect(page.getByRole("alertdialog").query()).toBeNull();
		expect(legacyLink().query()).toBeNull();
	});

	// Kills an email change saved before the count says whether it revokes any.
	test("an email change waits for the count", async () => {
		const { api } = userApi(0);
		const get = api.get;
		api.get = <T,>(path: string, signal?: AbortSignal) =>
			path.includes("/tokens")
				? new Promise<never>(() => {})
				: get<T>(path, signal);
		mount(api);

		await textbox("Name").fill("Muriel Courage");
		await expect.element(save()).toBeEnabled();
		await textbox("Email").fill("muriel@courage.example");
		await expect.element(save()).toBeDisabled();
	});

	// Kills a failed count that holds an email change back without a word,
	// and a Retry that asks nothing.
	test("a failed count says so, and Retry asks again", async () => {
		const { api } = userApi(2);
		const get = api.get;
		let fail = true;
		api.get = <T,>(path: string, signal?: AbortSignal) =>
			fail && path.includes("/tokens")
				? Promise.reject(new ApiError(500, "server", "{}"))
				: get<T>(path, signal);
		mount(api);

		const alert = page.getByRole("alert");
		await expect.element(alert).toHaveTextContent("Legacy tokens did not load");
		fail = false;
		await alert.getByRole("button", { name: "Retry" }).click();
		await expect.element(legacyLink()).toHaveTextContent("2 legacy tokens");
		expect(page.getByRole("alert").query()).toBeNull();
	});

	// Kills an admin signed out for changing another user's email, and that
	// user kept as the reader.
	test("another user's email change keeps the reader signed in", async () => {
		const { api, sent } = userApi(0);
		mount(api, { reader: ADMIN });

		await textbox("Email").fill("muriel@courage.example");
		await save().click();

		await expect.element(page.getByRole("status")).toHaveTextContent("Saved");
		expect(sent).toHaveLength(1);
		expect(signedOut).toBe(0);
		expect(kept).toEqual([]);
	});

	// Kills a link that waits for nothing, counts wrong, or is under 44 px on
	// a phone, an email change sent before the reader confirms, a Cancel that
	// sends it anyway, and a confirmation asked for a change that revokes
	// nothing.
	// The session ends only once the change is made.
	test("with some, a link counts them and an email change confirms first", async () => {
		const { api, sent } = userApi(2);
		mount(api);

		await expect.element(legacyLink()).toHaveTextContent("2 legacy tokens");
		// The browser's viewport is a phone's.
		expect(
			legacyLink().element().getBoundingClientRect().height,
		).toBeGreaterThanOrEqual(44);

		await textbox("Name").fill("Muriel Courage");
		await save().click();
		await expect.poll(() => sent.length).toBe(1);
		expect(page.getByRole("alertdialog").query()).toBeNull();

		await textbox("Email").fill("muriel@courage.example");
		await save().click();
		const dialog = page.getByRole("alertdialog");
		await expect
			.element(dialog)
			.toHaveTextContent("Your 2 legacy tokens will be revoked.");
		expect(sent).toHaveLength(1);

		await dialog.getByRole("button", { name: "Cancel" }).click();
		await expect.element(dialog).not.toBeInTheDocument();
		expect(sent).toHaveLength(1);
		await expect
			.element(textbox("Email"))
			.toHaveValue("muriel@courage.example");

		expect(signedOut).toBe(0);
		await save().click();
		await page.getByRole("button", { name: "Change email" }).click();
		await expect.poll(() => signedOut).toBe(1);
		expect(sent[1]?.body).toEqual({ email: "muriel@courage.example" });
	});
});

describe("the sections", () => {
	// Kills a rail drawn on a phone in place of the segmented control, and
	// the reverse on a wide screen.
	test("are a rail when wide and a segmented control on a phone", async () => {
		mount(userApi(0).api);
		const rail = page.getByRole("navigation", { name: "Account", exact: true });
		const segments = page.getByRole("navigation", { name: "Account sections" });

		await expect
			.element(segments.getByRole("link", { name: "Profile" }))
			.toBeVisible();
		expect(rail.query()).toBeNull();

		await page.viewport(1280, 800);
		await expect
			.element(rail.getByRole("link", { name: "Profile" }))
			.toBeVisible();
		expect(segments.query()).toBeNull();
		await page.viewport(414, 896);
	});
});
