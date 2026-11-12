import type { Resource } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import type { JsonUsage } from "../../../../types/bencher";
import PaymentMethod from "./PaymentMethod";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

/** The classic payment method for a plan with `plan`'s card and collection method. */
const shown = (plan: object) => {
	const usage = (() => ({ plan })) as unknown as Resource<null | JsonUsage>;
	const element = document.createElement("div");
	document.body.append(element);
	dispose = render(() => <PaymentMethod usage={usage} />, element);
	return element.textContent;
};

const CARD = { brand: "visa", last_four: "0417", exp_month: 8, exp_year: 2028 };

// Kills a card drawn for a plan that Stripe bills by invoice, a plan charged without a
// card drawn as invoiced, or a card that is not drawn.
test("names the invoice, the automatic payment, or the card", () => {
	const invoice = shown({ card: null, collection_method: "send_invoice" });
	expect(invoice).toContain("Invoice");
	expect(invoice).not.toContain("Last Four");
	dispose?.();

	const automatic = shown({
		card: null,
		collection_method: "charge_automatically",
	});
	expect(automatic).toContain("Automatic payment");
	expect(automatic).not.toContain("Invoice");
	dispose?.();

	const card = shown({ card: CARD, collection_method: "charge_automatically" });
	expect(card).toContain("Last Four: 0417");
	expect(card).not.toContain("Automatic payment");
});
