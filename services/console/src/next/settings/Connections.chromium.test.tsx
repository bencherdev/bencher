import { Show, createSignal } from "solid-js";
import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import {
	GitHubInstallationPermission,
	type JsonGitHubRepositories,
	type JsonLinkedRepository,
	LinkedRepositoryReason,
	LinkedRepositoryStatus,
} from "../../types/bencher";
import { ApiError } from "../api";
import Connections, { away, githubQuery, hold, prefetch } from "./Connections";
import { type Sent, bootstrapOf, fakeApi, mount } from "./testing";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	hold(undefined);
	vi.restoreAllMocks();
});

const AUTHORIZE = "https://github.example/login/oauth/authorize?state=s";
const INSTALL = "https://github.example/apps/bencher/installations/new?state=s";
const GRANT = "eyJhbGciOiJIUzI1NiJ9.eyJyZXBvIjoxfQ.Z3JhbnQ";

const LINKED: JsonLinkedRepository = {
	full_name: "pompeii-llc/hashbrown",
	status: LinkedRepositoryStatus.Connected,
	user: { uuid: "u", name: "Muriel Bagge", slug: "muriel-bagge" },
	linked: "2026-09-02T10:00:00Z",
};

const EXCHANGED: JsonGitHubRepositories = {
	project: "hashbrown",
	repositories: [
		{
			full_name: "pompeii-llc/hashbrown",
			installation: 1,
			installation_url: "https://github.example/organizations/pompeii-llc",
			permission: GitHubInstallationPermission.Granted,
			grant: GRANT,
		},
		{
			full_name: "bagge-farm/diner",
			installation: 2,
			installation_url: "https://github.example/settings/installations/2",
			permission: GitHubInstallationPermission.ApprovalRequired,
		},
	],
};

const URLS = { authorize: AUTHORIZE, install: INSTALL };

/** A read's answer, for a `read` stub. */
const answered = (data: unknown) => ({ data, headers: new Headers() }) as never;

/** Connections for `hashbrown`, its link already cached as `link`; `urls` answers the trip to GitHub. */
const open = (
	link: { linked_repository?: JsonLinkedRepository } | undefined,
	{
		answer,
		manage = true,
		urls = async () => answered(URLS),
	}: {
		answer?: (sent: Sent) => Promise<unknown>;
		manage?: boolean;
		urls?: () => Promise<never>;
	} = {},
) => {
	const reads: string[] = [];
	const { api, sent } = fakeApi(answer, async (path) => {
		reads.push(path);
		return urls();
	});
	const left = vi.spyOn(away, "to").mockImplementation(() => {});
	const mounted = mount(() => <Connections />, {
		api,
		path: "/hashbrown/connections",
		bootstrap: bootstrapOf({ manage }),
		cached: link ? [[githubQuery(api, "hashbrown").queryKey, link]] : [],
	});
	dispose = mounted.dispose;
	return { ...mounted, api, sent, reads, left };
};

const card = () => page.getByRole("region", { name: "GitHub" });
const button = (name: string) => card().getByRole("button", { name });

// Kills a Link repository that leaves without asking for the link's URLs,
// leaves for the install page, or stays; and one left disabled once it
// leaves, as a Back that restores the page from the browser's cache shows it.
test("with no link, Link repository leaves for GitHub's authorize page", async () => {
	const { reads, left } = open({});
	await button("Link repository").click();
	await expect.poll(() => left.mock.calls).toEqual([[AUTHORIZE]]);
	expect(reads).toEqual(["/v0/projects/hashbrown/github/urls"]);
	await expect.element(button("Link repository")).toBeEnabled();
});

// Kills a refused trip to GitHub that does not say why, or that leaves its
// button disabled, so the reader cannot try again.
test.each([
	["Link repository", {}],
	[
		"Install on GitHub",
		{
			linked_repository: {
				...LINKED,
				status: LinkedRepositoryStatus.Disconnected,
				reason: LinkedRepositoryReason.Uninstalled,
			},
		},
	],
] as const)(
	"a refused trip to GitHub says why and %s works again",
	async (name, link) => {
		const { left } = open(link, {
			urls: async () => {
				throw new ApiError(
					403,
					"forbidden",
					JSON.stringify({ message: "The project is not yours to link" }),
				);
			},
		});
		await button(name).click();
		await expect
			.element(card().getByRole("alert"))
			.toHaveTextContent(
				"Bencher did not start the link: The project is not yours to link",
			);
		await expect.element(button(name)).toBeEnabled();
		expect(left).not.toHaveBeenCalled();
	},
);

// Kills a linked card that drops the repository, its status, who linked it,
// or when, and one that offers Install on GitHub for a working link.
test("a connected link shows its repository, status, and who linked it when", async () => {
	open({ linked_repository: LINKED });
	await expect.element(card()).toHaveTextContent("pompeii-llc/hashbrown");
	await expect.element(card()).toHaveTextContent("Connected");
	await expect.element(card()).toHaveTextContent("Muriel Bagge");
	await expect.element(card()).toHaveTextContent("Sep 2, 2026");
	await expect.element(button("Unlink")).toBeVisible();
	expect(button("Install on GitHub").query()).toBeNull();
	expect(button("Link repository").query()).toBeNull();
});

// Kills a disconnected link drawn as connected, a reason dropped or named
// for another, and an Install on GitHub that leaves for the authorize page.
test.each([
	[LinkedRepositoryReason.Uninstalled, "App uninstalled"],
	[LinkedRepositoryReason.Suspended, "Installation suspended"],
	[LinkedRepositoryReason.Removed, "Removed from the installation"],
])(
	"a link disconnected as %s says why and offers Install on GitHub",
	async (reason, why) => {
		const { left } = open({
			linked_repository: {
				...LINKED,
				status: LinkedRepositoryStatus.Disconnected,
				reason,
			},
		});
		await expect.element(card()).toHaveTextContent("Disconnected");
		await expect.element(card()).toHaveTextContent(why);
		await button("Install on GitHub").click();
		await expect.poll(() => left.mock.calls).toEqual([[INSTALL]]);
	},
);

// Kills a server with no app drawn as a project with no link, which offers a
// Link repository the API would refuse, and the API's message swallowed.
test("a server with no app shows the API's message and no action", async () => {
	const message =
		"This server has no GitHub App, so a project cannot link a GitHub repository. Pass a token with contents write to `--ci-callback-token` instead.";
	const { api } = fakeApi(undefined, async () => {
		throw new ApiError(404, "not_found", JSON.stringify({ message }));
	});
	dispose = mount(() => <Connections />, {
		api,
		path: "/hashbrown/connections",
	}).dispose;
	await expect.element(card()).toHaveTextContent(message);
	expect(card().getByRole("button").elements()).toHaveLength(0);
});

// Kills a link asked for again, a round after the shell's request, when the
// section draws or prefetches over a refusal the prefetch already heard.
test("a refused link is asked for once, by the prefetch", async () => {
	const reads: string[] = [];
	const { api } = fakeApi(undefined, async (path) => {
		reads.push(path);
		throw new ApiError(404, "not_found", '{"message":"No GitHub App"}');
	});
	const [drawn, draw] = createSignal(false);
	const mounted = mount(
		() => (
			<Show when={drawn()}>
				<Connections />
			</Show>
		),
		{ api, path: "/hashbrown/connections" },
	);
	dispose = mounted.dispose;
	await prefetch(mounted.client, api, "hashbrown");
	draw(true);
	await expect.element(card()).toHaveTextContent("No GitHub App");
	await prefetch(mounted.client, api, "hashbrown");
	expect(reads).toEqual(["/v0/projects/hashbrown/github"]);
});

// Kills a request for the link, or a control, for a reader the API would
// refuse, and a missing read-only line.
test("a reader without manage sees the card read only and asks for nothing", async () => {
	hold(EXCHANGED);
	const { reads } = open(undefined, { manage: false });
	await expect.element(card()).toBeVisible();
	expect(card().getByRole("button").elements()).toHaveLength(0);
	expect(card().getByRole("link").elements()).toHaveLength(0);
	expect(page.getByText("Read only", { exact: true }).elements()).toHaveLength(
		1,
	);
	expect(reads).toEqual([]);
});

// Kills a link a reader without manage once saw, still drawn with its
// controls from the cache after the API stopped letting them read it.
test("a reader without manage sees no cached link and no control", async () => {
	open({ linked_repository: LINKED }, { manage: false });
	await expect.element(card()).toBeVisible();
	expect(card().getByRole("button").elements()).toHaveLength(0);
	expect(card().getByText("pompeii-llc/hashbrown").elements()).toHaveLength(0);
});

// Kills a grant sent by any method but PUT or in any other body, a list
// left in place once the link is made, and a card that does not show it.
test("Link sends the grant with PUT and shows the linked card", async () => {
	hold(EXCHANGED);
	const { sent } = open({}, { answer: async () => LINKED });
	await button("Link pompeii-llc/hashbrown").click();
	await expect.element(card()).toHaveTextContent("Connected");
	expect(sent).toEqual([
		{
			method: "PUT",
			path: "/v0/projects/hashbrown/github",
			body: { grant: GRANT },
		},
	]);
	expect(button("Link pompeii-llc/hashbrown").query()).toBeNull();
	await expect.element(button("Unlink")).toBeVisible();
});

// Kills repositories listed for a project other than the one they were
// listed for.
test("a list for another project is not shown", async () => {
	hold({ ...EXCHANGED, project: "bagge-farm" });
	open({});
	await expect.element(button("Link repository")).toBeVisible();
	expect(button("Link pompeii-llc/hashbrown").query()).toBeNull();
});

// Kills a Link offered for a repository with no grant, which the API would
// refuse, an approval left unmarked or without its way to GitHub, and the
// list without Install on GitHub for a repository that is not in it.
test("a repository waiting for approval has no Link and links to its installation", async () => {
	hold(EXCHANGED);
	const { left } = open({});
	const approve = card().getByRole("link", {
		name: "Approve on GitHub: bagge-farm/diner",
	});
	await expect
		.element(approve)
		.toHaveAttribute("href", "https://github.example/settings/installations/2");
	await expect.element(card()).toHaveTextContent("Approval required");
	expect(button("Link bagge-farm/diner").query()).toBeNull();
	await expect.element(button("Link pompeii-llc/hashbrown")).toBeVisible();
	// While the list shows, the link is already underway.
	expect(button("Link repository").query()).toBeNull();

	await button("Install on GitHub").click();
	await expect.poll(() => left.mock.calls).toEqual([[INSTALL]]);
});

// Kills a refused link that keeps a list whose grants the API no longer
// takes, leaving the reader no way to start over without a reload, and a
// refusal that does not say why.
test("a refused link drops the list and offers Link repository again", async () => {
	hold(EXCHANGED);
	open(
		{},
		{
			answer: async () => {
				throw new ApiError(
					400,
					"client",
					JSON.stringify({
						message: "The GitHub grant is not valid: ExpiredSignature",
					}),
				);
			},
		},
	);
	await button("Link pompeii-llc/hashbrown").click();
	await expect
		.element(card().getByRole("alert"))
		.toHaveTextContent(
			"Bencher did not link pompeii-llc/hashbrown: The GitHub grant is not valid: ExpiredSignature",
		);
	await expect.element(button("Link repository")).toBeVisible();
	expect(card().getByRole("list").query()).toBeNull();
});

// Kills a Link that stays live while its grant is on its way, so a second
// click sends a second request.
test("a second click on Link while it is sent sends nothing more", async () => {
	hold(EXCHANGED);
	const { sent } = open({}, { answer: () => new Promise(() => {}) });
	await button("Link pompeii-llc/hashbrown").click();
	await expect.poll(() => sent.length).toBe(1);
	// A click the browser delivers whatever the button's state.
	button("Link pompeii-llc/hashbrown").element().click();
	expect(sent).toHaveLength(1);
});

// Kills an unlink sent without the reader's confirmation, by the wrong
// method or path, one that waits for the API to show, and a refusal that
// leaves the link shown as gone or does not say why.
test("Unlink confirms, sends DELETE, and puts the link back on a refusal", async () => {
	let refuse: (error: unknown) => void = () => {};
	const { sent } = open(
		{ linked_repository: LINKED },
		{
			answer: () =>
				new Promise((_, reject) => {
					refuse = reject;
				}),
		},
	);
	await button("Unlink").click();
	const dialog = page.getByRole("alertdialog", {
		name: "Unlink pompeii-llc/hashbrown?",
	});
	await dialog.getByRole("button", { name: "Cancel" }).click();
	await expect.element(dialog).not.toBeInTheDocument();
	expect(sent).toEqual([]);

	await button("Unlink").click();
	await dialog.getByRole("button", { name: "Unlink" }).click();
	expect(sent).toEqual([
		{
			method: "DELETE",
			path: "/v0/projects/hashbrown/github",
			body: undefined,
		},
	]);
	await expect.element(button("Link repository")).toBeVisible();

	refuse(new ApiError(503, "server", '{"message":"Busy"}'));
	await expect
		.element(card().getByRole("alert"))
		.toHaveTextContent("Bencher did not unlink pompeii-llc/hashbrown: Busy");
	await expect.element(card()).toHaveTextContent("pompeii-llc/hashbrown");
	await expect.element(button("Unlink")).toBeVisible();
});
