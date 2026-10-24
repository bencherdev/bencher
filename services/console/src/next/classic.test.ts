import { describe, expect, test } from "vitest";
import { accountPage, shortcutPage } from "./account/paths";
import { classicRedirect } from "./classic";
import { CLASSIC_ROUTES, classicPath } from "./classic-routes";
import { organizationPageAt } from "./organization/paths";
import { pageName, parseConsolePath } from "./paths";
import { decodeQuery } from "./query/query";

const UUID = "0e5a0c32-8d3f-4b52-9a7e-3f8e0f1c2d4b";
const PARAMS = {
	project: "hashbrown",
	organization: "pompeii-llc",
	user: "muriel-bagge",
	alert: UUID,
	report: UUID,
	threshold: UUID,
	plot: UUID,
	key: UUID,
	metric: UUID,
	member: UUID,
	token: UUID,
	branch: "main",
	testbed: "ubuntu-latest",
	benchmark: "blake3",
	measure: "latency",
};

const redirect = (href: string) => {
	const { pathname, search } = new URL(href, "https://bencher.dev");
	return classicRedirect({ pathname, search });
};

/** Whether a page the move kept or built draws `pathname`. */
const drawn = (pathname: string) => {
	const project = parseConsolePath(pathname);
	if (project) {
		return pageName(project.rest) !== "NotFound";
	}
	return (
		accountPage(pathname) !== undefined ||
		organizationPageAt(pathname) !== undefined ||
		shortcutPage(pathname) !== undefined ||
		pathname === "/console" ||
		pathname === "/explore" ||
		/^\/perf\/[^/]+(\/embed)?$/.test(pathname)
	);
};

describe("every classic route", () => {
	// Kills a classic path left with no page and no redirect, and a redirect
	// that lands on a path no page draws.
	test.each(CLASSIC_ROUTES)("%s lands on a page", (route) => {
		const path = classicPath(route, PARAMS);
		const to = redirect(path);
		expect(drawn(new URL(to ?? path, "https://bencher.dev").pathname)).toBe(
			true,
		);
	});
});

describe("classicRedirect", () => {
	const project = "/console/projects/hashbrown";

	// Kills the classic perf page left in place, and its query dropped or left
	// in the classic spelling.
	test("sends the classic perf page to Explore with its query", () => {
		expect(
			redirect(
				`${project}/perf?branches=${UUID}&lower_value=true&upper_boundary=true&tab=branches&utm_source=github`,
			),
		).toBe(
			`${project}/explore?branches=${UUID}&metrics=value&metrics=lower_value`,
		);
		expect(redirect(`${project}/perf`)).toBe(`${project}/explore`);
	});

	// Kills a project root link to Explore, from before Runs moved there, left
	// on Runs, and a root with only tracking sent away from Runs.
	test("sends a project root that names Explore's query to Explore", () => {
		expect(redirect(`${project}?benchmarks=${UUID}&utm_source=github`)).toBe(
			`${project}/explore?benchmarks=${UUID}`,
		);
		expect(redirect(`${project}?utm_source=github`)).toBeUndefined();
		expect(redirect(`${project}/`)).toBeUndefined();
	});

	// Kills a classic path sent anywhere but the page that replaced it.
	test("sends each classic project path to the page that replaced it", () => {
		const cases: [string, string][] = [
			["reports?reports_page=2", "runs"],
			["branches/add", "branches"],
			["testbeds/add", "testbeds"],
			["benchmarks/add", "benchmarks"],
			["measures/add", "measures"],
			["thresholds/add?branch=main", "thresholds"],
			[`thresholds/${UUID}/edit`, `thresholds/${UUID}`],
			[`plots/${UUID}`, `explore?plot=${UUID}`],
			["keys/add", "keys"],
			[`keys/${UUID}`, "keys"],
			["metrics", "plots"],
		];
		for (const [from, to] of cases) {
			expect(redirect(`${project}/${from}`), from).toBe(`${project}/${to}`);
		}
	});

	// Kills a redirect for a path the console draws itself, which would cost a
	// round trip or loop.
	test("leaves a path the console draws alone", () => {
		for (const rest of [
			"",
			"runs",
			`reports/${UUID}`,
			"branches",
			"branches/main?head=x",
			"thresholds",
			`thresholds/${UUID}?model=x`,
			`alerts/${UUID}`,
			"plots",
			"keys",
			"settings",
			`metrics/${UUID}`,
			"explore?lower_value=true",
			`thresholds/${UUID}/edit/more`,
			"keys/add/more",
		]) {
			expect(redirect(`${project}/${rest}`), rest).toBeUndefined();
		}
	});

	// Kills the projects list left with nowhere to land.
	test("sends the classic projects list to the console", () => {
		expect(redirect("/console/projects")).toBe("/console");
		expect(redirect("/console/projects/")).toBe("/console");
	});

	// Kills the public list kept, and a classic public page left in place or
	// sent anywhere but the same page in the console.
	test("sends the public list to Explore and each public page to the console", () => {
		expect(redirect("/perf")).toBe("/explore");
		expect(redirect("/perf/")).toBe("/explore");
		expect(redirect(`/perf/hashbrown/reports/${UUID}?utm_source=github`)).toBe(
			`${project}/reports/${UUID}`,
		);
		expect(redirect("/perf/hashbrown/branches/main?head=x")).toBe(
			`${project}/branches/main`,
		);
		expect(redirect(`/perf/hashbrown/alerts/${UUID}`)).toBe(
			`${project}/alerts/${UUID}`,
		);
		expect(redirect(`/perf/hashbrown/plots/${UUID}`)).toBe(
			`${project}/explore?plot=${UUID}`,
		);
		expect(redirect("/perf/hashbrown/reports")).toBe(`${project}/runs`);
		expect(redirect(`/perf/hashbrown/metrics/${UUID}`)).toBe(
			`${project}/metrics/${UUID}`,
		);
	});

	// Kills a redirect away from the public plot or its embed, which draw in place.
	test("leaves the public plot and its embed in place", () => {
		expect(redirect("/perf/hashbrown?lower_value=true")).toBeUndefined();
		expect(redirect("/perf/hashbrown/embed?branches=x")).toBeUndefined();
	});

	// Kills an organization's classic page sent anywhere but its replacement,
	// checkout losing Stripe's return, and New organization's own page taken
	// for an organization named "add".
	test("sends each classic organization path to the page that replaced it", () => {
		const organization = "/console/organizations/pompeii-llc";
		expect(redirect(organization)).toBe(`${organization}/settings`);
		expect(redirect(`${organization}/projects`)).toBe(
			`${organization}/settings`,
		);
		expect(redirect(`${organization}/members/invite`)).toBe(
			`${organization}/members`,
		);
		expect(redirect(`${organization}/members/${UUID}`)).toBe(
			`${organization}/members`,
		);
		expect(
			redirect(`${organization}/checkout?checkout=cs_test&level=team`),
		).toBe(`${organization}/billing?checkout=cs_test&level=team`);
		for (const drawn of ["settings", "members", "billing", "projects/add"]) {
			expect(redirect(`${organization}/${drawn}`), drawn).toBeUndefined();
		}
		expect(redirect("/console/organizations/add")).toBeUndefined();
		expect(redirect("/console/organizations/add/projects")).toBe(
			"/console/organizations/add/settings",
		);
	});

	// Kills an account's classic page sent anywhere but its replacement, and a
	// shortcut or a page of its own taken for a user named after it.
	test("sends each classic account path to the page that replaced it", () => {
		const user = "/console/users/muriel-bagge";
		expect(redirect(user)).toBe(`${user}/settings`);
		expect(redirect(`${user}/keys/add`)).toBe(`${user}/keys`);
		expect(redirect(`${user}/keys/${UUID}`)).toBe(`${user}/keys`);
		expect(redirect(`${user}/tokens/add`)).toBe(`${user}/tokens`);
		expect(redirect(`${user}/tokens/${UUID}`)).toBe(`${user}/tokens`);
		for (const drawn of ["settings", "keys", "tokens", "apps", "help"]) {
			expect(redirect(`${user}/${drawn}`), drawn).toBeUndefined();
		}
		expect(redirect("/console/users/apps")).toBeUndefined();
		expect(redirect("/console/users/apps/keys/add")).toBe(
			"/console/users/apps/keys",
		);
		for (const shortcut of ["", "/settings", "/keys", "/keys/add", "/tokens"]) {
			expect(redirect(`/console/users${shortcut}`), shortcut).toBeUndefined();
		}
	});

	// Kills a redirect out of the pages the move keeps classic.
	test("leaves the console's root, onboarding, and help alone", () => {
		for (const path of [
			"/console",
			"/console/onboard/project",
			"/console/users/help",
			"/console/users/muriel-bagge/help",
			"/explore",
			"/auth/login",
		]) {
			expect(redirect(path), path).toBeUndefined();
		}
	});
});

describe("a pull request comment's classic link", () => {
	// Kills a released CLI's link to the perf page losing what it drew: the
	// branch, head, testbed, spec, benchmark, measure, window, and report, with
	// the bound an alert turned on.
	test("opens Explore on the same line", () => {
		const search = `?branches=${UUID}&heads=${UUID}&testbeds=${UUID}&specs=${UUID}&benchmarks=${UUID}&measures=${UUID}&start_time=1&end_time=2&report=${UUID}&upper_boundary=true&utm_medium=referral&utm_source=github&utm_content=comment&utm_campaign=pr%2Bcomments&utm_term=hashbrown`;
		const to = redirect(`/console/projects/hashbrown/perf${search}`);
		expect(to?.startsWith("/console/projects/hashbrown/explore?")).toBe(true);
		expect(decodeQuery(to?.split("?")[1] ?? "")).toEqual({
			...decodeQuery(search),
			metrics: [],
		});
		expect(to).not.toContain("utm_");
	});
});
