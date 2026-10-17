import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "./feedback.css";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test, vi } from "vitest";
import { page, userEvent } from "vitest/browser";
import FeedbackDialog, { type FeedbackAbout } from "./FeedbackDialog";
import type { Form } from "./report";
import { send } from "./send";

vi.mock("./send", () => ({ send: vi.fn() }));

const ABOUT = {
	project: "Muriel Bagge / Hashbrown",
	slug: "hashbrown",
	email: "muriel@nowhere.example",
};

let dispose: (() => void) | undefined;
let closed = 0;

const mount = (about: FeedbackAbout) => {
	dispose?.();
	document.body.replaceChildren();
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<FeedbackDialog
				about={about}
				onClose={() => {
					closed += 1;
				}}
			/>
		),
		root,
	);
};

beforeEach(() => {
	closed = 0;
	vi.mocked(send).mockReset();
	mount(ABOUT);
});

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
	vi.useRealTimers();
});

const message = () => page.getByRole("textbox", { name: "Message" });
const sendButton = () => page.getByRole("button", { name: "Send" });
/** The form the dialog last handed to Sentry. */
const sentForm = () => vi.mocked(send).mock.lastCall?.[0] as Form;

// Kills a report sent empty, a kind the choice does not reach, and a message
// that does not reach the call.
test("Send waits for a message and sends it with the kind chosen", async () => {
	vi.mocked(send).mockResolvedValue();
	await expect.element(sendButton()).toBeDisabled();
	await userEvent.fill(message().element(), "   ");
	await expect.element(sendButton()).toBeDisabled();

	await userEvent.fill(message().element(), "Pin a whole run to Plots");
	await page.getByRole("radio", { name: "Feedback" }).click();
	await sendButton().click();

	expect(sentForm()).toMatchObject({
		kind: "feedback",
		message: "Pin a whole run to Plots",
	});
	expect(sentForm().off).toEqual(new Set());
	expect(vi.mocked(send).mock.lastCall?.[1]).toMatchObject({
		slug: "hashbrown",
		email: "muriel@nowhere.example",
	});
});

// Kills an unticked item that still goes, and a box that cannot be unticked.
test("each item unticked stays out of the report", async () => {
	vi.mocked(send).mockResolvedValue();
	await userEvent.fill(message().element(), "Queued forever");
	await expect.element(page.getByRole("radio", { name: "Bug" })).toBeChecked();
	const boxes = page.getByRole("checkbox").elements();
	expect(boxes).toHaveLength(6);
	for (const box of boxes) {
		expect(box).toHaveProperty("checked", true);
	}

	await page.getByRole("checkbox", { name: /Session link/ }).click();
	await page.getByRole("checkbox", { name: /Your email/ }).click();
	await sendButton().click();

	expect(sentForm().kind).toBe("bug");
	expect(sentForm().off).toEqual(new Set(["session", "email"]));
});

// Kills a Project item offered on a page with no project, such as Profile.
test("a page outside a project offers no Project item", async () => {
	mount({ email: ABOUT.email });
	await expect
		.element(page.getByRole("checkbox", { name: /Your email/ }))
		.toBeVisible();
	expect(page.getByRole("checkbox", { name: /Project/ }).query()).toBeNull();
});

// Kills an image that never reaches the report, and one removed that still goes.
test("an added image goes with the report until removed", async () => {
	// A failure keeps the form up, so the same report can go again.
	vi.mocked(send).mockRejectedValue(new Error("blocked"));
	await userEvent.fill(message().element(), "Look at this");
	const file = new File([new Uint8Array(2048)], "runs-stuck.png", {
		type: "image/png",
	});
	await userEvent.upload(
		page.getByLabelText("Add an image", { exact: true }),
		file,
	);
	await expect.element(page.getByText("runs-stuck.png, 2 KB")).toBeVisible();
	await sendButton().click();
	await expect
		.poll(() => sentForm()?.image)
		.toMatchObject({
			filename: "runs-stuck.png",
			contentType: "image/png",
		});
	expect(sentForm().image?.data.byteLength).toBe(2048);

	await page.getByRole("button", { name: "Remove runs-stuck.png" }).click();
	await page.getByRole("button", { name: "Try again" }).click();
	await expect.poll(() => vi.mocked(send).mock.calls.length).toBe(2);
	expect(sentForm().image).toBeUndefined();
});

// Kills a sent report that leaves the form up, a Sent the focus and a screen
// reader never reach, and a dialog that never closes on its own or closes
// before the reader can see it went.
test("a sent report says so, then closes on its own", async () => {
	vi.mocked(send).mockResolvedValue();
	await userEvent.fill(message().element(), "Queued forever");
	vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout"] });
	await sendButton().click();

	await expect.element(page.getByRole("status")).toHaveTextContent("Sent");
	expect(message().query()).toBeNull();
	const done = page.getByRole("button", { name: "Done" });
	await expect.element(done).toHaveFocus();
	await expect.element(done).toHaveAccessibleDescription("Sent");
	vi.advanceTimersByTime(1_000);
	expect(closed).toBe(0);
	vi.advanceTimersByTime(3_000);
	expect(closed).toBe(1);
});

// Kills a failure that loses what the reader wrote, says nothing, or leaves no
// way to get the text to Bencher.
test("a report the browser blocked keeps the text and offers it another way", async () => {
	vi.mocked(send).mockRejectedValue(new Error("blocked"));
	await userEvent.fill(message().element(), "Queued forever");
	await sendButton().click();

	await expect
		.element(page.getByRole("alert"))
		.toHaveTextContent("Not sent: blocked by this browser");
	await expect.element(message()).toHaveValue("Queued forever");
	await expect
		.element(page.getByRole("link", { name: "Email help@bencher.dev" }))
		.toHaveAttribute("href", "mailto:help@bencher.dev");
	const copy = vi
		.spyOn(navigator.clipboard, "writeText")
		.mockResolvedValue(undefined);
	await page.getByRole("button", { name: "Copy text" }).click();
	await expect
		.element(page.getByRole("button", { name: "Copied" }))
		.toBeVisible();
	expect(copy).toHaveBeenCalledWith("Queued forever");

	vi.mocked(send).mockResolvedValue();
	await page.getByRole("button", { name: "Try again" }).click();
	await expect.element(page.getByRole("status")).toHaveTextContent("Sent");
});

// Kills a dialog Escape or Cancel cannot leave.
test("Escape and Cancel close it", async () => {
	await userEvent.keyboard("{Escape}");
	expect(closed).toBe(1);
	await page.getByRole("button", { name: "Cancel" }).click();
	expect(closed).toBe(2);
});
