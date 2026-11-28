import "./page.css";
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import CliCode from "./CliCode";

// The API's format: the prefix and 30 ASCII letters or digits.
const CODE = "bencher_code_pQ3v9xL2mT8rW4nB7cY1zH6fJ0dS5a";

let dispose: (() => void) | undefined;
afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const mount = (
	code: string | undefined,
	copy?: (code: string) => Promise<void>,
) => {
	const malformed = vi.fn();
	const root = document.createElement("main");
	document.body.append(root);
	dispose = render(
		() => <CliCode code={code} malformed={malformed} copy={copy} />,
		root,
	);
	return malformed;
};

// Kills a code page that sends the code anywhere but the clipboard, or never
// shows that it copied.
test("Copy puts the code on the clipboard and reads Copied", async () => {
	const copy = vi.fn(async () => {});
	const malformed = mount(CODE, copy);
	await expect
		.element(page.getByRole("heading", { level: 1 }))
		.toHaveTextContent("CLI code");
	await expect.element(page.getByText(CODE)).toBeVisible();
	await page.getByRole("button", { name: "Copy the code" }).click();
	expect(copy).toHaveBeenCalledExactlyOnceWith(CODE);
	await expect
		.element(page.getByRole("button", { name: "Code copied" }))
		.toHaveTextContent("Copied");
	expect(malformed).not.toHaveBeenCalled();
});

// Kills a page that shows, and offers to copy, anything its address carried
// that is not a code the API could have made, instead of going to the consent
// page's state for a link that is not valid.
for (const malformed of [
	undefined,
	"",
	"bencher_code_short",
	`${CODE}x`,
	CODE.replace("bencher_code_", "bencher_user_"),
	`${CODE.slice(0, -1)}-`,
]) {
	test(`no code to paste for ${JSON.stringify(malformed)}`, async () => {
		const left = mount(malformed);
		await expect.poll(() => left.mock.calls.length).toBe(1);
		expect(document.querySelector("main")?.textContent).toBe("");
		await expect.element(page.getByRole("button")).not.toBeInTheDocument();
	});
}
