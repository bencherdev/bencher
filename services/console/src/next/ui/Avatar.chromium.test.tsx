import "@bencherdev/ui/styles.css";
import Avatar from "@bencherdev/ui/Avatar";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";

let dispose: (() => void) | undefined;

afterEach(() => {
	dispose?.();
	dispose = undefined;
	document.body.replaceChildren();
});

const initialsOf = (name: string) => {
	const root = document.createElement("div");
	document.body.append(root);
	dispose?.();
	dispose = render(() => <Avatar name={name} />, root);
	return root.textContent;
};

// Kills initials cut by UTF-16 unit, which split a joined emoji, a flag, or a
// surrogate pair.
test("initials keep each character whole", () => {
	expect(initialsOf("👩‍💻 Dev")).toBe("👩‍💻D");
	expect(initialsOf("🇺🇸 Team")).toBe("🇺🇸T");
	expect(initialsOf("🚀 Rocket")).toBe("🚀R");
	expect(initialsOf("性能测试")).toBe("性能");
	expect(initialsOf("ada lovelace")).toBe("AL");
});
