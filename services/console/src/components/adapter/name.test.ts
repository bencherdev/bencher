import { readFileSync } from "node:fs";
import { describe, expect, test } from "vitest";
import { Language } from "../../i18n/ui";
import { Adapter } from "../../types/bencher";
import { adapterDocsUrl } from "./name";

const GROUP_RS = new URL(
	"../../../../../lib/bencher_comment/src/group.rs",
	import.meta.url,
);

/** Each adapter's heading id as the pull request comment links it. */
const commentFragments = () => {
	const source = readFileSync(GROUP_RS, "utf8");
	const body = source.slice(source.indexOf("fn adapter_docs("));
	const fragments = new Map<string, string>();
	for (const [, adapters, fragment] of body.matchAll(
		/((?:\|?\s*Adapter::\w+\s*)+)=>\s*\{?\s*\("[^"]+",\s*"([^"]+)"\)/g,
	)) {
		for (const [, name] of (adapters ?? "").matchAll(/Adapter::(\w+)/g)) {
			fragments.set(name ?? "", fragment ?? "");
		}
	}
	return fragments;
};

describe("adapterDocsUrl", () => {
	// Kills a heading id that drifts from the comment's link, or an adapter
	// whose link goes to the top of the page or to another harness.
	test("links the heading the comment links, for every adapter", () => {
		const fragments = commentFragments();
		expect([...fragments.keys()].sort()).toEqual(Object.keys(Adapter).sort());
		for (const [name, fragment] of fragments) {
			expect(
				adapterDocsUrl(Language.en, Adapter[name as keyof typeof Adapter]),
			).toBe(`/docs/explanation/adapters/#${fragment}`);
		}
	});

	// Kills a link that sends a reader of a translated page to the English one.
	test("keeps the reader's language", () => {
		expect(adapterDocsUrl(Language.de, Adapter.RustDivan)).toBe(
			"/de/docs/explanation/adapters/#-rust-divan",
		);
	});
});
