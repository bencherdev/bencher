import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import type {
	JsonConsoleOrganization,
	JsonConsoleUser,
	JsonConsoleUserProject,
} from "../../../types/bencher";
import ConsoleRedirect from "./ConsoleRedirect";

const { httpGet, navigate, hiddenRedirect } = vi.hoisted(() => ({
	httpGet: vi.fn(),
	navigate: vi.fn(),
	hiddenRedirect: vi.fn(),
}));

// Each mock names only what this graph imports, so knip still sees the rest unused.
vi.mock("@sentry/astro", () => ({ captureException: vi.fn() }));
vi.mock("../../../util/http", () => ({ httpGet }));
vi.mock("../../../util/valid", () => ({
	// Any value but undefined: the wasm has loaded.
	init_valid: async () => ({}),
	validJwt: () => true,
	validPlanLevel: (plan: undefined | null | string) => plan === "team",
	validEmail: () => true,
	validUserName: () => true,
}));
vi.mock("../../../util/auth", () => ({
	authUser: () => ({
		user: { uuid: "reader-uuid", slug: "reader" },
		token: "token",
	}),
	removeUser: vi.fn(),
}));
vi.mock("../../../util/url", () => ({
	useNavigate: () => navigate,
	useSearchParams: () => [
		Object.fromEntries(new URLSearchParams(window.location.search)),
		() => {},
	],
	hiddenRedirect,
	pathname: () => window.location.pathname,
}));

const acme: JsonConsoleOrganization = {
	uuid: "acme-uuid",
	name: "Acme",
	slug: "acme",
};
const other: JsonConsoleOrganization = {
	uuid: "other-uuid",
	name: "Other",
	slug: "other",
};
const classic: JsonConsoleUserProject = {
	uuid: "classic-uuid",
	name: "Classic",
	slug: "classic",
	organization: acme.uuid,
	bmf_version: 0,
	last_report: 1_000,
	active_alerts: 0,
};

let dispose: (() => void) | undefined;
const home = window.location.pathname;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	window.history.replaceState(null, "", home);
	localStorage.clear();
	vi.clearAllMocks();
});

/** Opens `/console` with `search`, the API answering `reader`, and waits for where it goes. */
const redirect = async (
	reader: Pick<JsonConsoleUser, "organizations" | "projects">,
	search = "",
) => {
	window.history.replaceState(null, "", `${home}${search}`);
	httpGet.mockResolvedValue({ data: { uuid: "reader-uuid", ...reader } });
	const container = document.createElement("div");
	document.body.append(container);
	dispose = render(
		() => <ConsoleRedirect apiUrl="http://api.test" />,
		container,
	);
	await vi.waitFor(() =>
		expect(navigate.mock.calls.length + hiddenRedirect.mock.calls.length).toBe(
			1,
		),
	);
	expect(httpGet).toHaveBeenCalledTimes(1);
	return (
		(navigate.mock.calls[0]?.[0] as string | undefined) ??
		`hidden ${hiddenRedirect.mock.calls[0]?.[0]}`
	);
};

// Kills onboarding sent to help, or a plan dropped on the way.
test("a reader with no projects onboards with their plan", async () => {
	expect(
		await redirect({ organizations: [acme], projects: [] }, "?plan=team"),
	).toBe("/console/onboard/project?plan=team");
});

// Kills an organization's notice dropped, or the organization not used.
test("a reader of one organization lands on its projects with the notice", async () => {
	expect(
		await redirect(
			{ organizations: [acme], projects: [classic] },
			"?notify_kind=ok&notify_text=Welcome",
		),
	).toBe(
		"/console/organizations/acme/projects?notify_kind=ok&notify_text=Welcome",
	);
});

// Kills a plan that no longer leads to billing.
test("a reader with a plan lands on their organization's billing", async () => {
	expect(
		await redirect(
			{ organizations: [acme], projects: [classic] },
			"?plan=team",
		),
	).toBe("/console/organizations/acme/billing?plan=team");
});

// Kills several organizations sent to one of them.
test("a reader of several organizations lands on the list", async () => {
	expect(
		await redirect({ organizations: [acme, other], projects: [classic] }),
	).toBe("/console/organizations");
});

// Kills help swapped with onboarding.
test("a reader of no organization is sent to help", async () => {
	expect(await redirect({ organizations: [], projects: [] })).toMatch(
		/^hidden \/console\/users\/reader\/help\?/,
	);
});

// Kills a version 0 project's notice dropped on the way to its classic page.
test("a newest version 0 project lands on its classic plots with the notice", async () => {
	const next: JsonConsoleUserProject = {
		...classic,
		uuid: "next-uuid",
		slug: "next",
		bmf_version: 1,
		last_report: 500,
	};
	expect(
		await redirect(
			{ organizations: [acme], projects: [next, classic] },
			"?notify_kind=ok&notify_text=Claimed",
		),
	).toBe("/console/projects/classic/plots?notify_kind=ok&notify_text=Claimed");
});
