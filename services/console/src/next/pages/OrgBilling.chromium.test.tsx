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
	vi.useRealTimers();
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
	collection_method: "charge_automatically",
	level: "pro",
	unit_amount: 0,
	// As the API sends them: an absent amount or bound is null.
	tiers: JSON.parse(`[
		{ "up_to": 250, "unit_amount": null, "flat_amount": 10000 },
		{ "up_to": 375, "unit_amount": null, "flat_amount": 15000 },
		{ "up_to": 500, "unit_amount": null, "flat_amount": 20000 },
		{ "up_to": null, "unit_amount": null, "flat_amount": 0 }
	]`),
	tiers_mode: "volume",
	// A dollar an hour, in cents a runner minute.
	runner_minute: 100 / 60,
	created: "2026-09-01T12:00:00Z",
	current_period_start: PERIOD.start_time,
	current_period_end: PERIOD.end_time,
	status: "active",
	cancel_at_period_end: false,
};

// A deal of 1,000 series for $2,500 a month, then $0.40 a series, with bare metal at
// two dollars an hour.
const ENTERPRISE = {
	...PLAN,
	level: "enterprise",
	tiers: JSON.parse(`[
		{ "up_to": 1000, "unit_amount": null, "flat_amount": 250000 },
		{ "up_to": null, "unit_amount": 40, "flat_amount": null }
	]`),
	tiers_mode: "graduated",
	runner_minute: 200 / 60,
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

// The peak of the term before, which the server keeps through a renewal.
const LAST_TERM = {
	term_end: "2026-10-01T12:00:00Z",
	active_series: 2_300,
	seen: "2026-09-20T12:00:00Z",
};

// A year's license for 2,000 Benchmark Series, read a month into its term.
const LICENSED = {
	organization: "o1",
	kind: "self_hosted_licensed",
	license: {
		key: KEY,
		organization: "o1",
		series: 2_000,
		issued_at: PERIOD.start_time,
		term_end: "2027-10-01T12:00:00Z",
		expiration: "2027-10-31T12:00:00Z",
	},
	start_time: "2026-10-02T12:00:00Z",
	end_time: PERIOD.end_time,
	active_series: 2_120,
	peak: {
		term_end: "2027-10-01T12:00:00Z",
		active_series: 2_480,
		seen: "2026-10-20T12:00:00Z",
	},
	previous_peak: LAST_TERM,
	runner_minutes: 4_210,
} as unknown as JsonUsage;

/** The browser's clock, frozen at `time`. */
const at = (time: string) => {
	vi.useFakeTimers({ toFake: ["Date"] });
	vi.setSystemTime(Date.parse(time));
};

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
		/** The organization the cache holds, Pompeii LLC unless given; none when null. */
		organization?: JsonOrganization | null;
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
		...(organization === undefined ? {} : { organization }),
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

	// Kills an Enterprise plan drawn as Pro, its own overage band drawn as a
	// conversation, its bill estimated as a volume price, or its bare metal billed at
	// the published rate.
	test("an Enterprise plan prices its own bands, bill, and bare metal", async () => {
		open({
			...PRO,
			active_series: 1_200,
			plan: ENTERPRISE,
		} as JsonUsage);
		const plan = region("Enterprise Plan");
		await expect
			.element(plan.getByRole("img", { name: "1,200 of 1,000 active series" }))
			.toBeVisible();
		await expect.element(plan).toHaveTextContent("$10.60 at $2.00 an hour");
		await expect.element(plan).toHaveTextContent("Estimated bill$2,590.60");
		const rows = page
			.getByRole("table", { name: "Enterprise price bands" })
			.getByRole("row")
			.elements()
			.slice(1)
			.map((row) => row.textContent);
		expect(rows).toEqual([
			"0 to 1,000$2,500 a month",
			"1,001 or more$0.40 a seriesThis period",
		]);
		await expect
			.element(region("Danger"))
			.toHaveTextContent(
				"Enterprise ends with this period on Nov 1, 2026, and Pompeii LLC moves to the Free plan.",
			);
		expect(region("Pro Plan").query()).toBeNull();
	});

	// Kills an Enterprise price without bands estimated as a conversation, which only
	// Pro's open band is.
	test("an Enterprise plan without bands shows no estimate", async () => {
		open({
			...PRO,
			plan: { ...ENTERPRISE, tiers: null, tiers_mode: null },
		} as unknown as JsonUsage);
		const plan = region("Enterprise Plan");
		await expect.element(plan).toHaveTextContent("Estimated bill-");
		expect(plan.getByRole("link", { name: "Get in touch" }).query()).toBeNull();
	});

	// Kills an Enterprise band without a price drawn as a conversation, which only
	// Pro's open band is.
	test("an Enterprise band without a price shows a dash", async () => {
		open({
			...PRO,
			plan: {
				...ENTERPRISE,
				tiers: JSON.parse(`[
					{ "up_to": 100, "unit_amount": null, "flat_amount": null },
					{ "up_to": null, "unit_amount": 40, "flat_amount": null }
				]`),
			},
		} as unknown as JsonUsage);
		const bands = page.getByRole("table", { name: "Enterprise price bands" });
		await expect.element(bands).toBeVisible();
		const rows = bands
			.getByRole("row")
			.elements()
			.slice(1)
			.map((row) => row.textContent);
		expect(rows).toEqual(["0 to 100-", "101 or more$0.40 a seriesThis period"]);
		expect(
			bands.getByRole("link", { name: "Get in touch" }).query(),
		).toBeNull();
	});

	// Kills a plan billed by invoice drawn with a card it does not have, or left with no
	// way to the billing portal.
	test("a plan billed by invoice names the invoice and opens the portal", async () => {
		const { sent, left } = open(
			{
				...PRO,
				plan: {
					...ENTERPRISE,
					card: null,
					customer: null,
					collection_method: "send_invoice",
				},
			} as unknown as JsonUsage,
			{
				answer: async () => ({ url: "https://billing.stripe.com/p/session/y" }),
			},
		);
		const payment = region("Payment method");
		await expect.element(payment).toHaveTextContent("Invoice");
		expect(payment.element().textContent).not.toContain("Expires");
		await button("Manage billing").click();

		await expect
			.poll(() => left.mock.calls)
			.toEqual([["https://billing.stripe.com/p/session/y"]]);
		expect(sent).toEqual([
			{
				method: "POST",
				path: "/v0/organizations/o1/plan/portal",
				body: undefined,
			},
		]);
	});
});

describe("a paid plan without a card", () => {
	// Kills a plan that Stripe charges with no card, such as by a bank debit, drawn as
	// billed by invoice.
	test("a plan charged without a card names automatic payment", async () => {
		open({
			...PRO,
			plan: {
				...ENTERPRISE,
				card: null,
				collection_method: "charge_automatically",
			},
		} as unknown as JsonUsage);
		const payment = region("Payment method");
		await expect.element(payment).toHaveTextContent("Automatic payment");
		expect(payment.element().textContent).not.toContain("Invoice");
		await expect.element(button("Manage billing")).toBeVisible();
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

	// Kills an upgrade sent for another organization, one that still names a
	// level, or one that stays on the page once Stripe answers.
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
				body: { organization: "o1" },
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
	const RETURN = "/organizations/pompeii-llc/billing?checkout=cs_x";
	const USAGE = "/v0/organizations/pompeii-llc/usage";
	const enrolled = () => page.getByRole("status").getByText("Enrolled in Pro");

	// Kills a return that sends anything but its session, or none at all.
	test("records the plan with the return's session alone", async () => {
		const { sent } = open(FREE, {
			path: RETURN,
			read: async () => answered(PRO),
			answer: async () => PLAN,
		});

		await expect.element(enrolled()).toBeVisible();
		expect(sent).toEqual([
			{
				method: "POST",
				path: "/v0/organizations/pompeii-llc/plan",
				body: { checkout: "cs_x" },
			},
		]);
	});

	// Kills "Enrolled" drawn over the plan from before the checkout, with its
	// Start trial still live, before the usage read after the plan answers, and
	// any usage read but that one.
	test("says Enrolled only with the usage read after the plan is recorded", async () => {
		// The usage read once the plan is posted waits for the test.
		const held: ((usage: JsonUsage) => void)[] = [];
		// Counted apart from `sent`, which a read during mount finds unassigned.
		let reads = 0;
		const { sent, history } = open(FREE, {
			path: RETURN,
			read: async (path) => {
				expect(path).toBe(USAGE);
				reads += 1;
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
		await new Promise((resolve) => setTimeout(resolve, 100));
		expect(reads).toBe(1);
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

	// Kills a license box that drops the term, the active series against the
	// license's, the peak or its date, the last term's peak, its date, or its
	// term end, or the bare metal minutes, a grace warned in the term, and a key
	// box that leads with the key rather than the organization's UUID, or calls
	// a valid key not valid.
	test("a licensed install shows its license, then the UUID and its key", async () => {
		open(LICENSED);
		const license = region("Enterprise License");
		await expect
			.element(license)
			.toHaveTextContent("Oct 1, 2026 to Oct 1, 2027");
		await expect
			.element(
				license.getByRole("img", { name: "2,120 of 2,000 active series" }),
			)
			.toBeVisible();
		await expect.element(license).toHaveTextContent("Peak2,480");
		await expect.element(license).toHaveTextContent("Oct 20, 2026");
		await expect.element(license).toHaveTextContent("Last term2,300");
		await expect
			.element(license)
			.toHaveTextContent("Sep 20, 2026, term ended Oct 1, 2026");
		await expect.element(license).toHaveTextContent("4,210 min");
		expect(license.getByRole("alert").query()).toBeNull();
		expect(license.getByRole("status").query()).toBeNull();
		expect(rowLabels()).toEqual(["Organization UUID", "License key"]);
		await expect
			.element(page.getByRole("textbox", { name: "License key" }))
			.toHaveValue(KEY);
		await expect.element(button("Replace key")).toBeDisabled();
		expect(region("License key").getByText("Not valid").query()).toBeNull();
	});

	// Kills a license in its grace drawn without a warning, one that drops the
	// end of the term or the expiration, or a grace read from the reader's clock
	// rather than the server's.
	test("a license in its grace by the server's time warns with the term end and the expiration", async () => {
		at(PERIOD.end_time);
		open({ ...LICENSED, end_time: "2027-10-10T12:00:00Z" } as JsonUsage);
		const license = region("Enterprise License");
		await expect
			.element(license.getByRole("status"))
			.toHaveTextContent(
				"The term ended Oct 1, 2027. The license expires Oct 31, 2027.",
			);
	});

	// Kills a grace read from the reader's clock, which the report responses
	// from the server do not send.
	test("a reader's clock past the term end warns of no grace the server has not reached", async () => {
		at("2027-10-10T12:00:00Z");
		open(LICENSED);
		await expect
			.element(region("Enterprise License"))
			.toHaveTextContent("Peak2,480");
		expect(region("Enterprise License").getByRole("status").query()).toBeNull();
	});

	// Kills a license with no count yet that shows a peak, or one with no
	// earlier term that shows a last term.
	test("a license with no counted report shows no peak", async () => {
		open({
			...LICENSED,
			active_series: 0,
			peak: undefined,
			previous_peak: undefined,
		} as unknown as JsonUsage);
		const license = region("Enterprise License");
		await expect
			.element(license.getByRole("img", { name: "0 of 2,000 active series" }))
			.toBeVisible();
		await expect.element(license).toHaveTextContent("Peak-");
		await expect.element(license).not.toHaveTextContent("Last term");
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
		expect(region("License").getByText("Not valid").query()).toBeNull();
		await expect.element(month).not.toHaveTextContent("Last term");
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

	// Kills an expired license, which the API reads as Free, left out of the key
	// box, so the reader has no key to replace, or left without a word that it is
	// not valid.
	test("a Free install with an expired license offers its key for replacing", async () => {
		open(UNLICENSED, {
			organization: { ...ORGANIZATION, license: "old.key.jwt" },
		});
		await expect.element(region("This month")).toBeVisible();
		await expect
			.element(page.getByRole("textbox", { name: "License key" }))
			.toHaveValue("old.key.jwt");
		await expect.element(button("Replace key")).toBeDisabled();
		await expect
			.element(region("License key").getByText("Not valid"))
			.toBeVisible();
	});

	// Kills a key box that fills itself again with the stored key once the reader clears it,
	// as the last character of a key deleted one at a time does.
	test("a cleared key box stays empty and offers nothing to save", async () => {
		open(LICENSED);
		const box = page.getByRole("textbox", { name: "License key" });
		await expect.element(box).toHaveValue(KEY);
		await box.fill("e");
		await expect.element(box).toHaveValue("e");
		await box.fill("");

		await expect.element(box).toHaveValue("");
		await expect.element(button("Replace key")).toBeDisabled();
	});

	// Kills a key box that copies the organization's key once, as it mounts, so a key
	// that the organization's read brings after the usage never shows.
	test("a stored key that arrives after the usage still fills the key box", async () => {
		open(UNLICENSED, {
			organization: null,
			read: async (path) => {
				if (path === "/v0/organizations/pompeii-llc") {
					return answered({ ...ORGANIZATION, license: "old.key.jwt" });
				}
				return new Promise<never>(() => {});
			},
		});
		await expect
			.element(page.getByRole("textbox", { name: "License key" }))
			.toHaveValue("old.key.jwt");
		await expect
			.element(region("License key").getByText("Not valid"))
			.toBeVisible();
	});

	// Kills the last term's peak dropped once the license has expired, or one
	// drawn for an install that never had a counted term.
	test("a Free install shows the last term's peak when there is one", async () => {
		open({ ...UNLICENSED, previous_peak: LAST_TERM } as unknown as JsonUsage);
		const month = region("This month");
		await expect.element(month).toHaveTextContent("Last term2,300");
		await expect
			.element(month)
			.toHaveTextContent("Sep 20, 2026, term ended Oct 1, 2026");
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
	// A Self-Hosted deal billed by invoice, read a month into its term by the server's time.
	const CLOUD_LICENSED = {
		organization: "o1",
		kind: "cloud_self_hosted_licensed",
		plan: { ...ENTERPRISE, card: null, collection_method: "send_invoice" },
		license: { ...LICENSED.license, organization: "s1" },
		start_time: PERIOD.start_time,
		end_time: PERIOD.end_time,
	} as unknown as JsonUsage;
	/** The license kept on Cloud, as the server reads it at `now`. */
	const readAt = (now: string) =>
		({ ...CLOUD_LICENSED, end_time: now }) as unknown as JsonUsage;

	// Kills a key offered for editing where the API refuses to change it, or
	// a key box that names this organization rather than the install's.
	test("its key is read only, for the install's UUID, beside the invoice", async () => {
		open(CLOUD_LICENSED);
		await expect
			.element(page.getByRole("textbox", { name: "License key" }))
			.toHaveAttribute("readonly");
		await expect
			.element(page.getByRole("textbox", { name: "License key" }))
			.toHaveValue(KEY);
		expect(button("Replace key").query()).toBeNull();
		await expect
			.element(region("License key"))
			.toHaveTextContent("Self-Hosted organization UUIDs1");
		await expect.element(region("Payment method")).toHaveTextContent("Invoice");
	});

	// Kills a license whose count, term end, or expiration is not shown, since the
	// Self-Hosted server counts nothing on Cloud.
	test("it shows the Benchmark Series it covers, its term end, and its expiration", async () => {
		open(CLOUD_LICENSED);
		const license = region("Enterprise License");
		await expect.element(license).toHaveTextContent("Benchmark Series2,000");
		await expect.element(license).toHaveTextContent("Term endsOct 1, 2027");
		await expect.element(license).toHaveTextContent("ExpiresOct 31, 2027");
		expect(license.getByRole("status").query()).toBeNull();
	});

	// Kills a Copy for the key that copies anything but the key, or none at all.
	test("Copy copies the license key", async () => {
		const copy = vi
			.spyOn(navigator.clipboard, "writeText")
			.mockResolvedValue(undefined);
		open(CLOUD_LICENSED);
		await button("Copy the license key").click();

		expect(copy).toHaveBeenCalledWith(KEY);
		await expect.element(button("Copied the license key")).toBeVisible();
	});

	// Kills a grace told by the browser's clock, or not told at all.
	test("a license in its grace by the server's time says when it expires", async () => {
		at("2026-01-01T12:00:00Z");
		open(readAt("2027-10-10T12:00:00Z"));
		await expect
			.element(region("Enterprise License").getByRole("status"))
			.toHaveTextContent(
				"The term ended Oct 1, 2027. The license expires Oct 31, 2027.",
			);
	});

	// Kills an expired license drawn as one in its grace, or as valid, or told by the
	// browser's clock.
	test("a license expired by the server's time says so", async () => {
		at("2027-10-10T12:00:00Z");
		open(readAt("2027-11-05T12:00:00Z"));
		await expect
			.element(region("Enterprise License").getByRole("status"))
			.toHaveTextContent("The license expired Oct 31, 2027.");
		expect(page.getByText("The term ended").query()).toBeNull();
		await expect
			.element(page.getByRole("textbox", { name: "License key" }))
			.toHaveValue(KEY);
	});

	// Kills a plan whose license the read left out drawn as its heading alone, or with a key
	// to copy.
	test("a plan whose license the read left out shows its term and no license", async () => {
		open({ ...CLOUD_LICENSED, license: null } as unknown as JsonUsage);
		await expect
			.element(region("Enterprise License"))
			.toHaveTextContent("Oct 1 to Nov 1, 2026");
		await expect.element(region("Payment method")).toHaveTextContent("Invoice");
		await expect.element(region("License key")).toHaveTextContent("No license");
		expect(
			page.getByRole("textbox", { name: "License key" }).query(),
		).toBeNull();
		expect(button("Copy the license key").query()).toBeNull();
	});

	// Kills a self-serve checkout or cancel offered for a contract that sales provisioned.
	test("it offers no checkout and no cancel", async () => {
		open(CLOUD_LICENSED);
		await expect.element(region("Enterprise License")).toBeVisible();
		expect(button("Start 1-month free trial").query()).toBeNull();
		expect(button("Cancel plan").query()).toBeNull();
		expect(button("Resume plan").query()).toBeNull();
		expect(region("Plan").query()).toBeNull();
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

describe("an organization the API will not show the reader", () => {
	// Kills a refusal drawn as a failure to retry, and one hidden behind the
	// permission the API answers false, as it does for every reader it refuses.
	test.each([
		[403, "forbidden", false],
		[404, "not_found", false],
		[404, "not_found", null],
	] as const)(
		"a refusal (%i, %s) of the organization, manage %s, is not found, with no Retry",
		async (status, kind, manage) => {
			open(null, {
				manage,
				organization: null,
				read: async (path) => {
					if (path === "/v0/organizations/pompeii-llc") {
						throw new ApiError(status, kind, "{}");
					}
					return new Promise<never>(() => {});
				},
			});
			await expect
				.element(page.getByRole("main").getByText("Organization not found"))
				.toBeVisible();
			expect(page.getByRole("alert").query()).toBeNull();
			expect(page.getByRole("button", { name: "Retry" }).query()).toBeNull();
		},
	);

	// Kills a usage the API will not give a manager, a plan it cannot find or
	// a refusal, drawn as the organization not found.
	test.each([
		[403, "forbidden"],
		[404, "not_found"],
	] as const)(
		"a usage the API refuses (%i, %s) says Billing did not load, and retries",
		async (status, kind) => {
			const reads: string[] = [];
			open(null, {
				read: async (path) => {
					reads.push(path);
					if (path.endsWith("/usage")) {
						throw new ApiError(status, kind, "{}");
					}
					return new Promise<never>(() => {});
				},
			});
			await expect
				.element(page.getByRole("alert"))
				.toHaveTextContent("Billing did not load");
			expect(page.getByText("Organization not found").query()).toBeNull();
			await button("Retry").click();
			await expect
				.poll(() => reads.filter((path) => path.endsWith("/usage")).length)
				.toBe(2);
		},
	);
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
		.toHaveAttribute("href", "/console/organizations/pompeii-llc/billing");
});
