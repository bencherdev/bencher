import type { Resource } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import type { Reader } from "../../../next/reader";
import type { JsonUsage } from "../../../types/bencher";
import { fmtDate } from "../../../util/convert";
import { CloudSelfHostedLicensedPanel, PlanCancelResume } from "./BillingPanel";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

/** The classic panel for a license kept on Cloud, for `license`, which the read may leave out. */
const shown = (license: object | null) => {
	const usage = (() => ({
		kind: "cloud_self_hosted_licensed",
		plan: {
			card: null,
			collection_method: "send_invoice",
			current_period_start: "2026-10-01T12:00:00Z",
			current_period_end: "2026-11-01T12:00:00Z",
		},
		license,
	})) as unknown as Resource<null | JsonUsage>;
	const element = document.createElement("div");
	document.body.append(element);
	dispose = render(
		() => <CloudSelfHostedLicensedPanel onboard={false} usage={usage} />,
		element,
	);
	return element;
};

const copies = (element: HTMLElement) =>
	element.querySelectorAll('a[title="Copy to clipboard"]').length;

// Kills a plan whose license the read left out drawn without its term, or with a key to copy,
// and a license that is no longer drawn with its key to copy.
test("a plan whose license the read left out shows its term and no license", () => {
	const left = shown(null);
	expect(left.textContent).toContain(fmtDate("2026-10-01T12:00:00Z"));
	expect(left.textContent).toContain(fmtDate("2026-11-01T12:00:00Z"));
	expect(left.textContent).toContain("No license");
	expect(copies(left)).toBe(0);
	dispose?.();

	const kept = shown({
		key: "eyJhbGciOiJFUzI1NiJ9.eyJzdWIiOiJzMSJ9.c2lnbmF0dXJl",
		organization: "s1",
		series: 2_000,
		issued_at: "2026-10-01T12:00:00Z",
		term_end: "2027-10-01T12:00:00Z",
		expiration: "2027-10-31T12:00:00Z",
	});
	expect(kept.textContent).not.toContain("No license");
	expect(kept.textContent).toContain("s1");
	expect(copies(kept)).toBe(2);
});

/** The classic cancel and resume control, for a metered plan at `level` whose end is scheduled when `ending`. */
const control = (level: string, ending: boolean) => {
	const usage = (() => ({
		kind: "cloud_metered",
		organization: "o1",
		plan: {
			level,
			cancel_at_period_end: ending,
			current_period_end: "2026-11-01T12:00:00Z",
		},
	})) as unknown as Resource<null | JsonUsage>;
	const element = document.createElement("div");
	document.body.append(element);
	dispose = render(
		() => (
			<PlanCancelResume
				user={{} as Reader}
				usage={usage}
				handleRefresh={() => {}}
			/>
		),
		element,
	);
	return element;
};

const buttons = (element: HTMLElement) =>
	[...element.querySelectorAll("button")].map((button) => button.textContent);
const contact = (element: HTMLElement) =>
	element.querySelector('a[href="mailto:enterprise@bencher.dev"]')?.textContent;

// Kills a cancel or resume offered for an Enterprise plan, a contact link in its place that is
// not the Enterprise mailto, or a Pro plan that loses its controls to it.
test("an Enterprise plan offers Contact us in place of Cancel plan and Resume plan", () => {
	const pro = control("pro", false);
	expect(buttons(pro)).toEqual(["Cancel plan"]);
	expect(contact(pro)).toBeUndefined();
	dispose?.();

	const proEnding = control("pro", true);
	expect(buttons(proEnding)).toEqual(["Resume plan"]);
	expect(contact(proEnding)).toBeUndefined();
	dispose?.();

	const enterprise = control("enterprise", false);
	expect(buttons(enterprise)).toEqual([]);
	expect(contact(enterprise)).toBe("Contact us");
	dispose?.();

	const enterpriseEnding = control("enterprise", true);
	expect(buttons(enterpriseEnding)).toEqual([]);
	expect(contact(enterpriseEnding)).toBe("Contact us");
});
