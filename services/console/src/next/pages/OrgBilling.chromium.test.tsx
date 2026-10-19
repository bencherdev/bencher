import { afterEach, describe, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import type { JsonOrganization, JsonUsage } from "../../types/bencher";
import { ApiError } from "../api";
import { away, manageQuery, usageQuery } from "../organization/billing";
import { ORGANIZATION, mount } from "../organization/testing";
import { fakeApi } from "../settings/testing";
import OrgBilling from "./OrgBilling";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	vi.restoreAllMocks();
	localStorage.clear();
});

const PERIOD = {
	start_time: "2026-10-01T12:00:00Z",
	end_time: "2026-11-01T12:00:00Z",
};

const PLAN = {
	organization: "o1",
	customer: {
		uuid: "u",
		name: "Muriel Bagge",
		email: "muriel@nowhere.example",
	},
	card: { brand: "visa", last_four: "0417", exp_month: 8, exp_year: 2028 },
	level: "pro",
	unit_amount: 0,
	// As the API sends them: an absent amount or bound is null.
	tiers: JSON.parse(`[
		{ "up_to": 250, "unit_amount": null, "flat_amount": 10000 },
		{ "up_to": 375, "unit_amount": null, "flat_amount": 15000 },
		{ "up_to": 500, "unit_amount": null, "flat_amount": 20000 },
		{ "up_to": null, "unit_amount": null, "flat_amount": 0 }
	]`),
	created: "2026-09-01T12:00:00Z",
	current_period_start: PERIOD.start_time,
	current_period_end: PERIOD.end_time,
	status: "active",
	cancel_at_period_end: false,
};

const PRO = {
	organization: "o1",
	kind: "cloud_metered",
	plan: PLAN,
	...PERIOD,
	active_series: 212,
	runner_minutes: 318,
} as unknown as JsonUsage;

const FREE = {
	organization: "o1",
	kind: "cloud_free",
	...PERIOD,
	metrics: 186,
	runner_minutes: 42,
} as unknown as JsonUsage;

const KEY = "eyJhbGciOiJFUzI1NiJ9.eyJzdWIiOiJvMSJ9.c2lnbmF0dXJl";

const LICENSED = {
	organization: "o1",
	kind: "self_hosted_licensed",
	license: {
		key: KEY,
		organization: "o1",
		level: "enterprise",
		entitlements: 2_000_000,
		issued_at: PERIOD.start_time,
		expiration: "2027-10-01T12:00:00Z",
	},
	start_time: PERIOD.start_time,
	end_time: "2027-10-01T12:00:00Z",
	metrics: 1_212_480,
	runner_minutes: 4_210,
} as unknown as JsonUsage;

const UNLICENSED = {
	organization: "o1",
	kind: "self_hosted_free",
	...PERIOD,
	metrics: 48_201,
	runner_minutes: 610,
} as unknown as JsonUsage;

/** Billing for Pompeii LLC, with `usage` and the reader's `manage` already cached. */
const open = (
	usage: JsonUsage | null,
	{
		manage = true,
		answer = async () => undefined,
		read,
		path = "/organizations/pompeii-llc/billing",
		organization,
	}: {
		manage?: boolean | null;
		answer?: Parameters<typeof fakeApi>[0];
		read?: Parameters<typeof fakeApi>[1];
		path?: string;
		organization?: JsonOrganization;
	} = {},
) => {
	const { api, sent } = fakeApi(answer, read);
	const left = vi.spyOn(away, "to").mockImplementation(() => {});
	const cached: [unknown[], unknown][] = [];
	if (usage) {
		cached.push([usageQuery(api, "pompeii-llc").queryKey, usage]);
	}
	if (manage !== null) {
		cached.push([manageQuery(api, "pompeii-llc").queryKey, manage]);
	}
	const mounted = mount(() => <OrgBilling />, {
		api,
		path,
		cached,
		...(organization ? { organization } : {}),
	});
	dispose = mounted.dispose;
	return { ...mounted, api, sent, left };
};

/** The page below the rail: only the heading's text, and no placeholder, when nothing else is drawn. */
const body = () =>
	document.querySelector("main .setbody .ui-skeleton")
		? "placeholder"
		: document.querySelector("main .setbody")?.textContent;

/** A read's answer, for a `read` stub. */
const answered = (data: unknown) => ({ data, headers: new Headers() }) as never;

const region = (name: string) => page.getByRole("region", { name });
const button = (name: string) => page.getByRole("button", { name });
const refused = (message: string) =>
	new ApiError(409, "client", JSON.stringify({ message }));

describe("a paid plan", () => {
	// Kills a plan box that drops the period, the usage against the plan's
	// series, the bare metal cost, the estimate, or the date it is charged.
	test("the plan box holds the period, usage, and the estimated bill", async () => {
		open(PRO);
		const plan = region("Pro Plan");
		await expect.element(plan).toHaveTextContent("Oct 1 to Nov 1, 2026");
		await expect
			.element(plan.getByRole("img", { name: "212 of 250 active series" }))
			.toBeVisible();
		await expect.element(plan).toHaveTextContent("318 min");
		await expect.element(plan).toHaveTextContent("$5.30 at $1.00 an hour");
		await expect.element(plan).toHaveTextContent("$105.30");
		await expect.element(plan).toHaveTextContent("Charged Nov 1");
	});

	// Kills a band table that marks no band, the wrong band, or prices the open one.
	test("the price bands mark this period's band", async () => {
		open(PRO);
		const rows = page
			.getByRole("table", { name: "Pro price bands" })
			.getByRole("row")
			.elements()
			.slice(1)
			.map((row) => row.textContent);
		expect(rows).toEqual([
			"0 to 250$100 a monthThis period",
			"251 to 375$150 a month",
			"376 to 500$200 a month",
			"501 or moreGet in touch",
		]);
	});

	// Kills a payment method that names the wrong card or drops its expiry.
	test("the payment method names the card and its expiry", async () => {
		open(PRO);
		const payment = region("Payment method");
		await expect.element(payment).toHaveTextContent("Visa ending 0417");
		await expect.element(payment).toHaveTextContent("Expires 08/2028");
	});

	// Kills an Update card that asks for another organization's portal, never
	// asks, or stays on the page once Stripe answers.
	test("Update card asks for the portal and leaves for it", async () => {
		const { sent, left } = open(PRO, {
			answer: async () => ({ url: "https://billing.stripe.com/p/session/x" }),
		});
		await button("Update card").click();

		await expect
			.poll(() => left.mock.calls)
			.toEqual([["https://billing.stripe.com/p/session/x"]]);
		expect(sent).toEqual([
			{
				method: "POST",
				path: "/v0/organizations/o1/plan/portal",
				body: undefined,
			},
		]);
	});

	// Kills a refused portal request that leaves anyway, or does not say why.
	test("a refused portal request says why and stays", async () => {
		const { left } = open(PRO, {
			answer: async () => {
				throw refused("Plan not found");
			},
		});
		await button("Update card").click();

		await expect
			.element(region("Payment method").getByRole("alert"))
			.toHaveTextContent(
				"Bencher did not open the billing portal: Plan not found",
			);
		expect(left).not.toHaveBeenCalled();
	});

	// Kills a cancel sent without the reader's confirmation, a Keep Pro that
	// sends it, a cancel that asks to end anything but this period, a page
	// that still offers to cancel once it is scheduled, and focus lost with
	// the confirmation.
	test("Cancel plan confirms, then schedules the end and offers Resume", async () => {
		const { sent } = open(PRO, {
			answer: async ({ body }) => ({ ...PLAN, ...(body as object) }),
		});
		const danger = region("Danger");
		await expect
			.element(danger)
			.toHaveTextContent(
				"Pro ends with this period on Nov 1, 2026, and Pompeii LLC moves to the Free plan.",
			);
		await danger.getByRole("button", { name: "Cancel plan" }).click();
		const dialog = page.getByRole("alertdialog", { name: "Cancel Pro?" });
		await expect
			.element(dialog)
			.toHaveAccessibleDescription("Private projects lock on Nov 1.");

		await dialog.getByRole("button", { name: "Keep Pro" }).click();
		await expect.element(dialog).not.toBeInTheDocument();
		expect(sent).toEqual([]);

		await danger.getByRole("button", { name: "Cancel plan" }).click();
		await dialog.getByRole("button", { name: "Cancel Pro" }).click();

		await expect.element(button("Resume plan")).toBeVisible();
		expect(sent).toEqual([
			{
				method: "PATCH",
				path: "/v0/organizations/o1/plan",
				body: { cancel_at_period_end: true },
			},
		]);
		await expect
			.element(region("Pro Plan"))
			.toHaveTextContent("Ends Nov 1, 2026");
		expect(region("Danger").query()).toBeNull();
		expect(document.activeElement?.id).toBe("sec-plan");
	});

	// Kills a cached plan painted as if it were current: a visit reads the
	// usage again and draws its answer.
	test("a visit reads the usage again and draws the fresh answer", async () => {
		const reads: string[] = [];
		open(PRO, {
			read: async (path) => {
				reads.push(path);
				return answered({ ...PRO, active_series: 300 });
			},
		});

		await expect
			.element(page.getByRole("img", { name: "300 of 250 active series" }))
			.toBeVisible();
		expect(reads).toContain("/v0/organizations/pompeii-llc/usage");
	});

	// Kills the payer's name and email cached from the plan a cancel answers with.
	test("a cancel caches the plan without its payer", async () => {
		const { api, client } = open(PRO, {
			answer: async ({ body }) => ({ ...PLAN, ...(body as object) }),
		});
		await button("Cancel plan").click();
		await page
			.getByRole("alertdialog", { name: "Cancel Pro?" })
			.getByRole("button", { name: "Cancel Pro" })
			.click();
		await expect.element(button("Resume plan")).toBeVisible();

		const cached = client.getQueryData<{ plan: object }>(
			usageQuery(api, "pompeii-llc").queryKey,
		);
		expect(cached?.plan).not.toHaveProperty("customer");
		expect(cached?.plan).toHaveProperty("cancel_at_period_end", true);
	});

	// Kills a confirmation left open by a cancel that worked, which a later
	// Resume shows again.
	test("a cancel, then Resume, leaves no confirmation open", async () => {
		open(PRO, {
			answer: async ({ body }) => ({ ...PLAN, ...(body as object) }),
		});
		await button("Cancel plan").click();
		await page
			.getByRole("alertdialog", { name: "Cancel Pro?" })
			.getByRole("button", { name: "Cancel Pro" })
			.click();
		await button("Resume plan").click();

		await expect
			.element(region("Danger").getByRole("button", { name: "Cancel plan" }))
			.toBeVisible();
		expect(page.getByRole("alertdialog").query()).toBeNull();
	});

	// Kills a Resume that sends anything but clearing the scheduled end, or
	// leaves the page offering to resume.
	test("Resume plan clears the scheduled end", async () => {
		const { sent } = open(
			{ ...PRO, plan: { ...PLAN, cancel_at_period_end: true } } as JsonUsage,
			{ answer: async ({ body }) => ({ ...PLAN, ...(body as object) }) },
		);
		await button("Resume plan").click();

		await expect
			.element(region("Danger").getByRole("button", { name: "Cancel plan" }))
			.toBeVisible();
		expect(sent).toEqual([
			{
				method: "PATCH",
				path: "/v0/organizations/o1/plan",
				body: { cancel_at_period_end: false },
			},
		]);
	});

	// Kills a refused cancel that closes the confirmation as if it worked, or
	// does not say why.
	test("a refused cancel says why and keeps the confirmation", async () => {
		open(PRO, {
			answer: async () => {
				throw refused("Stripe is unavailable");
			},
		});
		await button("Cancel plan").click();
		const dialog = page.getByRole("alertdialog", { name: "Cancel Pro?" });
		await dialog.getByRole("button", { name: "Cancel Pro" }).click();

		await expect
			.element(dialog.getByRole("alert"))
			.toHaveTextContent(
				"Bencher did not cancel the plan: Stripe is unavailable",
			);
		expect(button("Resume plan").query()).toBeNull();
	});

	// Kills a trial billed as a paid period.
	test("a trial's estimated bill is nothing", async () => {
		open({
			...PRO,
			plan: { ...PLAN, status: "trialing" },
		} as JsonUsage);
		await expect
			.element(region("Pro Plan"))
			.toHaveTextContent("Estimated bill$0.00");
	});
});

describe("the Free plan", () => {
	// Kills plan cards that mark the wrong plan current, or drop this month's usage.
	test("the plan cards lead, with Free current, then this month's usage", async () => {
		open(FREE);
		const cards = region("Plan");
		await expect
			.element(cards.getByRole("article", { name: "Free" }))
			.toHaveTextContent("Current");
		expect(
			cards.getByRole("article", { name: "Pro" }).element().textContent,
		).not.toContain("Current");
		await expect
			.element(
				cards
					.getByRole("article", { name: "Enterprise" })
					.getByRole("link", { name: "Contact us" }),
			)
			.toHaveAttribute("href", "mailto:enterprise@bencher.dev");
		const month = region("This month");
		await expect.element(month).toHaveTextContent("Oct 1 to Nov 1, 2026");
		await expect.element(month).toHaveTextContent("Metrics186");
		await expect.element(month).toHaveTextContent("Bare metal42 min");
	});

	// Kills an upgrade sent for another organization or plan, or one that
	// stays on the page once Stripe answers.
	test("Pro's card starts a checkout and leaves for it", async () => {
		const { sent, left } = open(FREE, {
			answer: async () => ({
				session: "cs_x",
				url: "https://checkout.stripe.com/c/pay/cs_x",
			}),
		});
		await button("Start 1-month free trial").click();

		await expect
			.poll(() => left.mock.calls)
			.toEqual([["https://checkout.stripe.com/c/pay/cs_x"]]);
		expect(sent).toEqual([
			{
				method: "POST",
				path: "/v0/checkout",
				body: { organization: "o1", level: "pro" },
			},
		]);
	});

	// Kills the pricing page's Pro link that no longer starts a checkout, or
	// one that starts it again on the next visit.
	test("?plan=pro starts the checkout once", async () => {
		const { sent, left, history } = open(FREE, {
			path: "/organizations/pompeii-llc/billing?plan=pro",
			read: async () => answered(FREE),
			answer: async () => ({
				session: "cs_x",
				url: "https://checkout.stripe.com/x",
			}),
		});

		await expect
			.poll(() => left.mock.calls)
			.toEqual([["https://checkout.stripe.com/x"]]);
		expect(sent.map(({ path }) => path)).toEqual(["/v0/checkout"]);
		expect(history.get()).toBe("/organizations/pompeii-llc/billing");
	});

	// Kills a checkout started from a cached Free plan that this visit's read
	// shows was already bought.
	test("?plan=pro starts nothing when the fresh read is no longer Free", async () => {
		const reads: string[] = [];
		const { sent, left, history } = open(FREE, {
			path: "/organizations/pompeii-llc/billing?plan=pro",
			read: async (path) => {
				reads.push(path);
				return answered(PRO);
			},
		});

		await expect.element(region("Pro Plan")).toBeVisible();
		expect(reads).toContain("/v0/organizations/pompeii-llc/usage");
		expect(history.get()).toBe("/organizations/pompeii-llc/billing");
		expect(left).not.toHaveBeenCalled();
		expect(sent).toEqual([]);
	});

	// Kills a refused checkout that leaves anyway, or does not say why.
	test("a refused checkout says why and stays", async () => {
		const { left } = open(FREE, {
			answer: async () => {
				throw refused("Organization already has a plan");
			},
		});
		await button("Start 1-month free trial").click();

		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent(
				"Bencher did not start the checkout: Organization already has a plan",
			);
		expect(left).not.toHaveBeenCalled();
	});
});

describe("Stripe's return", () => {
	const RETURN = "/organizations/pompeii-llc/billing?checkout=cs_x&level=pro";
	const USAGE = "/v0/organizations/pompeii-llc/usage";
	const enrolled = () => page.getByRole("status").getByText("Enrolled in Pro");

	// Kills "Enrolled" drawn over the plan from before the checkout, with its
	// Start trial still live, before the usage read after the plan answers.
	test("says Enrolled only with the usage read after the plan is recorded", async () => {
		// The usage read once the plan is posted waits for the test.
		const held: ((usage: JsonUsage) => void)[] = [];
		const { sent, history } = open(FREE, {
			path: RETURN,
			read: async (path) => {
				expect(path).toBe(USAGE);
				return sent.length === 0
					? answered(FREE)
					: new Promise((resolve) =>
							held.push((usage) => resolve(answered(usage))),
						);
			},
		});

		await expect.poll(() => held.length).toBeGreaterThan(0);
		expect(sent.map(({ path }) => path)).toEqual([
			"/v0/organizations/pompeii-llc/plan",
		]);
		expect(history.get()).toBe("/organizations/pompeii-llc/billing");
		await new Promise((resolve) => setTimeout(resolve, 100));
		expect(enrolled().query()).toBeNull();
		expect(button("Start 1-month free trial").query()).toBeNull();

		for (const release of held) {
			release(PRO);
		}
		await expect.element(enrolled()).toBeVisible();
		await expect.element(region("Pro Plan")).toBeVisible();
	});

	// Kills a replayed return, whose plan the API already holds, shown as a failure.
	test("a return whose plan is already recorded says Enrolled", async () => {
		open(FREE, {
			path: RETURN,
			read: async () => answered(PRO),
			answer: async () => {
				throw refused(
					"Plan ((QueryOrganization { .. }, QueryPlan { .. })) has conflict: Organization already has a plan",
				);
			},
		});

		await expect.element(enrolled()).toBeVisible();
		await expect.element(region("Pro Plan")).toBeVisible();
		expect(page.getByRole("alert").query()).toBeNull();
	});

	// Kills a refusal that says nothing, any other conflict taken for a recorded
	// plan, and a failure that starts the checkout again by itself.
	test("a refused return says why and starts no checkout", async () => {
		const { sent, history } = open(FREE, {
			path: RETURN,
			read: async () => answered(FREE),
			answer: async ({ path }) => {
				if (path.endsWith("/plan")) {
					throw refused("Plan (..) has conflict: UNIQUE constraint failed");
				}
				return { session: "cs_y", url: "https://checkout.stripe.com/y" };
			},
		});

		await expect
			.element(page.getByRole("alert"))
			.toHaveTextContent(
				"Bencher did not record the Pro plan: Plan (..) has conflict: UNIQUE constraint failed",
			);
		await expect.element(button("Start 1-month free trial")).toBeEnabled();
		await new Promise((resolve) => setTimeout(resolve, 100));
		expect(sent.map(({ path }) => path)).toEqual([
			"/v0/organizations/pompeii-llc/plan",
		]);
		expect(history.get()).toBe("/organizations/pompeii-llc/billing");
	});
});

describe("self-hosted", () => {
	const rowLabels = () =>
		[
			...document.querySelectorAll(
				'[aria-labelledby="sec-key"] .formrow .fl > :first-child',
			),
		].map((label) => label.textContent);

	// Kills a license box that drops the term, the metrics against the
	// entitlements, or the bare metal minutes, and a key box that leads with
	// the key rather than the organization's UUID.
	test("a licensed install shows its license, then the UUID and its key", async () => {
		open(LICENSED);
		const license = region("Enterprise License");
		await expect
			.element(license)
			.toHaveTextContent("Oct 1, 2026 to Oct 1, 2027");
		await expect
			.element(
				license.getByRole("img", { name: "1,212,480 of 2,000,000 metrics" }),
			)
			.toBeVisible();
		await expect.element(license).toHaveTextContent("4,210 min");
		expect(rowLabels()).toEqual(["Organization UUID", "License key"]);
		await expect
			.element(page.getByRole("textbox", { name: "License key" }))
			.toHaveValue(KEY);
		await expect.element(button("Replace key")).toBeDisabled();
	});

	// Kills a replaced key sent to another organization, or a page that keeps
	// showing the old license once the new one is saved.
	test("Replace key sends the new key, then reads the license again", async () => {
		const reads: string[] = [];
		const { sent } = open(LICENSED, {
			answer: async ({ body }) => ({ uuid: "o1", ...(body as object) }),
			read: (path) => {
				reads.push(path);
				return new Promise(() => {});
			},
		});
		await page
			.getByRole("textbox", { name: "License key" })
			.fill("new.key.jwt");
		await button("Replace key").click();

		await expect
			.poll(() => reads)
			.toContain("/v0/organizations/pompeii-llc/usage");
		expect(sent).toEqual([
			{
				method: "PATCH",
				path: "/v0/organizations/o1",
				body: { license: "new.key.jwt" },
			},
		]);
	});

	// Kills an unlicensed install drawn without its usage, without Contact us,
	// with the key before the UUID, or with a Save key that sends nothing.
	test("an unlicensed install shows this month, then the UUID and an empty key", async () => {
		open(UNLICENSED);
		const month = region("This month");
		await expect.element(month).toHaveTextContent("Metrics48,201");
		await expect.element(month).toHaveTextContent("Bare metal610 min");
		await expect
			.element(region("License").getByRole("link", { name: "Contact us" }))
			.toHaveAttribute("href", "mailto:enterprise@bencher.dev");
		expect(rowLabels()).toEqual(["Organization UUID", "License key"]);
		await expect
			.element(page.getByRole("textbox", { name: "License key" }))
			.toHaveValue("");
		await expect.element(button("Save key")).toBeDisabled();
	});

	// Kills a refused key that does not say why.
	test("a refused key says why", async () => {
		open(UNLICENSED, {
			answer: async () => {
				throw refused("Invalid license");
			},
		});
		await page
			.getByRole("textbox", { name: "License key" })
			.fill("bad.key.jwt");
		await button("Save key").click();

		await expect
			.element(region("License").getByRole("alert"))
			.toHaveTextContent(
				"Bencher did not save the license key: Invalid license",
			);
	});

	// Kills a Copy that copies anything but the organization's UUID.
	test("Copy copies the organization's UUID", async () => {
		const copy = vi
			.spyOn(navigator.clipboard, "writeText")
			.mockResolvedValue(undefined);
		open(UNLICENSED);
		await button("Copy the organization UUID").click();

		expect(copy).toHaveBeenCalledWith("o1");
	});
});

describe("a license kept on Cloud", () => {
	const CLOUD_LICENSED = {
		organization: "o1",
		kind: "cloud_self_hosted_licensed",
		plan: PLAN,
		license: { ...LICENSED.license, organization: "s1" },
		...PERIOD,
	} as unknown as JsonUsage;

	// Kills a key offered for editing where the API refuses to change it, or
	// a key box that names this organization rather than the install's.
	test("its key is read only, for the install's UUID, beside the card", async () => {
		open(CLOUD_LICENSED);
		await expect
			.element(page.getByRole("textbox", { name: "License key" }))
			.toHaveAttribute("readonly");
		expect(button("Replace key").query()).toBeNull();
		await expect.element(region("License key")).toHaveTextContent("s1");
		await expect
			.element(region("Payment method"))
			.toHaveTextContent("Visa ending 0417");
	});
});

// Kills an expired or invalid license that leaves the reader no place to replace its key.
test("a license the API cannot read still offers its key for replacing", async () => {
	open(null, {
		organization: { ...ORGANIZATION, license: "old.key.jwt" },
		read: async (path) => {
			throw path.endsWith("/usage")
				? new ApiError(402, "client", '{"message":"License expired"}')
				: new Error(path);
		},
	});
	await expect
		.element(page.getByRole("textbox", { name: "License key" }))
		.toHaveValue("old.key.jwt");
	await expect.element(region("License key")).toHaveTextContent("o1");
	expect(page.getByRole("alert").query()).toBeNull();
});

// Kills a load error left without a way back, or a Retry that reads the usage no more.
test("a usage that does not load says so, and Retry reads it again", async () => {
	const reads: string[] = [];
	open(null, {
		read: async (path) => {
			reads.push(path);
			throw new ApiError(500, "server", "");
		},
	});
	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Billing did not load");
	await button("Retry").click();

	await expect
		.poll(() => reads.filter((path) => path.endsWith("/usage")).length)
		.toBe(2);
});

describe("only what the reader can do", () => {
	// Kills billing drawn, or its usage asked for, for a reader the API would refuse.
	test("a reader who cannot manage the organization sees no billing", async () => {
		const reads: string[] = [];
		open(null, {
			manage: false,
			read: (path) => {
				reads.push(path);
				return new Promise(() => {});
			},
		});
		await expect
			.element(page.getByRole("heading", { level: 1, name: "Billing" }))
			.toBeVisible();
		expect(body()).toBe("Settings/Billing");
		expect(reads).not.toContain("/v0/organizations/pompeii-llc/usage");
	});

	// Kills a refusal of the usage, which a reader without manage gets on a
	// cold load before the permission answers, shown as a load error.
	test("a cold load for a reader without manage shows no error", async () => {
		let allow: (allowed: boolean) => void = () => {};
		const reads: string[] = [];
		open(null, {
			manage: null,
			read: (path) => {
				reads.push(path);
				if (path.endsWith("/usage")) {
					return Promise.reject(new ApiError(403, "forbidden", ""));
				}
				return new Promise((resolve) => {
					allow = (allowed) =>
						resolve({ data: { allowed }, headers: new Headers() } as never);
				});
			},
		});
		await expect
			.poll(() => reads)
			.toContain("/v0/organizations/pompeii-llc/usage");
		await new Promise((resolve) => requestAnimationFrame(resolve));
		expect(page.getByRole("alert").query()).toBeNull();

		allow(false);
		await expect.poll(body).toBe("Settings/Billing");
		expect(page.getByRole("alert").query()).toBeNull();
	});

	// Kills controls drawn before the API has said what the reader may do.
	test("nothing is drawn until the permission is known", async () => {
		open(PRO, { manage: null });
		await expect
			.element(page.getByRole("heading", { level: 1, name: "Billing" }))
			.toBeVisible();
		expect(document.querySelectorAll("main button, main section").length).toBe(
			0,
		);
	});
});

// Kills a rail that marks the wrong section current or keeps Billing on the classic console.
test("the rail marks Billing current, in the new console", async () => {
	await page.viewport(1280, 800);
	open(PRO);
	const billing = page
		.getByRole("navigation", { name: "Organization settings", exact: true })
		.getByRole("link", { name: "Billing" });
	await expect.element(billing).toHaveAttribute("aria-current", "page");
	await expect
		.element(billing)
		.toHaveAttribute("href", "/next/console/organizations/pompeii-llc/billing");
});
