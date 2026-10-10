import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../report/report.css";
import { createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import AlertActions from "./AlertActions";

const NAME = "blake3 n=0 Latency";
const SEP_12 = Date.parse("2026-09-12T15:00:00Z");

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const mount = (props: Partial<Parameters<typeof AlertActions>[0]>) => {
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<AlertActions
				name={NAME}
				status="active"
				justNow={false}
				modified={SEP_12}
				{...props}
			/>
		),
		root,
	);
	return root;
};

// Kills a Dismiss named for nothing, one drawn for a reader who cannot use
// it, and an active alert offered Reactivate.
test("an active alert offers Dismiss to an editor and nothing to a viewer", async () => {
	const onDismiss = vi.fn();
	mount({ onDismiss, onReactivate: vi.fn() });
	await page
		.getByRole("button", { name: `Dismiss the alert on ${NAME}` })
		.click();
	expect(onDismiss).toHaveBeenCalledOnce();
	await expect
		.element(page.getByRole("button", { name: /^Reactivate/ }))
		.not.toBeInTheDocument();

	dispose?.();
	document.body.replaceChildren();
	const root = mount({});
	expect(root.textContent).toBe("");
});

// Kills a dismissed alert that hides when it was dismissed, says "just now"
// for an old dismissal or a date for a new one, or offers no way back.
test.each([
	[true, "Dismissed just now"],
	[false, "Dismissed Sep 12"],
])(
	"a dismissed alert, just now %s, says %s and offers Reactivate",
	async (justNow, text) => {
		const onReactivate = vi.fn();
		mount({ status: "dismissed", justNow, onDismiss: vi.fn(), onReactivate });
		await expect.element(page.getByText(text)).toBeVisible();
		await page
			.getByRole("button", { name: `Reactivate the alert on ${NAME}` })
			.click();
		expect(onReactivate).toHaveBeenCalledOnce();
		await expect
			.element(page.getByRole("button", { name: /^Dismiss/ }))
			.not.toBeInTheDocument();
	},
);

// Kills Reactivate drawn for a reader who cannot use it.
test("a viewer sees a dismissed alert's status and no Reactivate", async () => {
	const root = mount({ status: "dismissed" });
	expect(root.textContent).toBe("Dismissed Sep 12");
	await expect.element(page.getByRole("button")).not.toBeInTheDocument();
});

// Kills a silenced alert without its reason, or with a Reactivate the API would refuse.
test("a silenced alert says why and offers nothing", async () => {
	const root = mount({
		status: "silenced",
		onDismiss: vi.fn(),
		onReactivate: vi.fn(),
	});
	expect(root.textContent).toBe("Silenced: the branch head was replaced");
	await expect.element(page.getByRole("button")).not.toBeInTheDocument();
});

// Kills focus dropped to the page when the control a reader pressed gives way to its undo.
test("focus moves to the control that undoes the change", async () => {
	const [status, setStatus] = createSignal<"active" | "dismissed">("active");
	const root = document.createElement("div");
	document.body.append(root);
	dispose = render(
		() => (
			<AlertActions
				name={NAME}
				status={status()}
				justNow
				modified={SEP_12}
				onDismiss={() => setStatus("dismissed")}
				onReactivate={() => setStatus("active")}
			/>
		),
		root,
	);
	await page
		.getByRole("button", { name: `Dismiss the alert on ${NAME}` })
		.click();
	await expect
		.element(
			page.getByRole("button", { name: `Reactivate the alert on ${NAME}` }),
		)
		.toHaveFocus();
	await page
		.getByRole("button", { name: `Reactivate the alert on ${NAME}` })
		.click();
	await expect
		.element(page.getByRole("button", { name: `Dismiss the alert on ${NAME}` }))
		.toHaveFocus();
});
