import "@bencherdev/ui/styles.css";
import "./page.css";
import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { USER_KEY } from "../reader";
import { boot } from "./main";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;
// Every box filled, so the page asks for the plot as it boots.
const DRAWABLE = `?branches=${uuid(1)}&testbeds=${uuid(3)}&benchmarks=${uuid(5)}&measures=${uuid(7)}`;
const home = `${location.pathname}${location.search}`;

afterEach(() => {
	vi.unstubAllGlobals();
	history.replaceState(null, "", home);
	localStorage.clear();
	document.body.replaceChildren();
});

/**
 * The public plot booted on a project the API refuses to anyone: the plot's
 * refusal answers first, the project's next, and the reader's own read of the
 * project never does. Returns every alert the page ever drew.
 */
const refusedInOrder = () => {
	let refuseProject = () => {};
	const projectRefused = new Promise<void>((resolve) => {
		refuseProject = resolve;
	});
	const refusal = () =>
		new Response('{"message":"Not Found"}', { status: 404 });
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: string, init?: RequestInit) => {
			const url = new URL(input);
			if (url.pathname.endsWith("/perf")) {
				setTimeout(refuseProject, 50);
				return refusal();
			}
			if (new Headers(init?.headers).has("Authorization")) {
				return new Promise<never>(() => {});
			}
			await projectRefused;
			return refusal();
		}),
	);
	history.replaceState(null, "", `${location.pathname}${DRAWABLE}`);
	const mount = document.createElement("div");
	mount.className = "console";
	mount.dataset.apiUrl = "http://api.test";
	mount.dataset.slug = "hashbrown";
	document.body.append(mount);
	const alerts: string[] = [];
	new MutationObserver(() => {
		for (const alert of mount.querySelectorAll('[role="alert"]')) {
			alerts.push(alert.textContent ?? "");
		}
	}).observe(mount, { childList: true, subtree: true, characterData: true });
	boot(mount);
	return { mount, alerts };
};

// Kills a plot refusal drawn before the project's own refusal settles what the
// page shows: the page would flash an error over a project no one may read.
test("a plot refused before its project shows no error, then locks", async () => {
	const { alerts } = refusedInOrder();

	await expect
		.element(
			page.getByRole("heading", { level: 1, name: "Private or not found" }),
		)
		.toBeVisible();
	expect(alerts).toEqual([]);
});

// Kills the same flash for a stored reader, whose own read of the project
// follows the refusal.
test("a plot refused before its project shows no error while the reader is checked", async () => {
	localStorage.setItem(
		USER_KEY,
		JSON.stringify({
			token: "tok",
			user: { uuid: "u", slug: "muriel-bagge" },
			expiration: "2999-01-01T00:00:00Z",
		}),
	);
	const { mount, alerts } = refusedInOrder();

	await expect
		.poll(() => mount.querySelector('main.pu-in[aria-busy="true"]'))
		.not.toBeNull();
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(alerts).toEqual([]);
});

// Kills a member's own read of the project that the plot's refusal does not
// wait on: the member's refused query would never say so.
test("a member's refused query says so once the member's read of the project answers", async () => {
	localStorage.setItem(
		USER_KEY,
		JSON.stringify({
			token: "tok",
			user: { uuid: "u", slug: "muriel-bagge" },
			expiration: "2999-01-01T00:00:00Z",
		}),
	);
	const answer = (status: number, body: unknown) =>
		new Response(JSON.stringify(body), { status });
	vi.stubGlobal(
		"fetch",
		vi.fn(async (input: string, init?: RequestInit) => {
			const member = new Headers(init?.headers).has("Authorization");
			if (new URL(input).pathname.endsWith("/perf")) {
				return member
					? answer(400, { message: "Bad Request" })
					: answer(404, { message: "Not Found" });
			}
			return member
				? answer(200, {
						name: "Hashbrown",
						slug: "hashbrown",
						visibility: "private",
						bmf_version: 1,
					})
				: answer(404, { message: "Not Found" });
		}),
	);
	history.replaceState(null, "", `${location.pathname}${DRAWABLE}`);
	const mount = document.createElement("div");
	mount.className = "console";
	mount.dataset.apiUrl = "http://api.test";
	mount.dataset.slug = "hashbrown";
	document.body.append(mount);
	boot(mount);

	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("The API refused this query");
	await expect
		.element(page.getByRole("heading", { level: 1, name: "Hashbrown" }))
		.toBeVisible();
});
