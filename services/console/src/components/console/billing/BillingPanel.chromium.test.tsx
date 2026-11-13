import type { Resource } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import type { JsonUsage } from "../../../types/bencher";
import { fmtDate } from "../../../util/convert";
import { CloudSelfHostedLicensedPanel } from "./BillingPanel";

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
