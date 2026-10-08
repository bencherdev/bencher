import type { PageName } from "./paths";

/** The modules a page of the new console loads, as built. */
interface Preloads {
	/** Every module the app's entry and the page script load before the first render. */
	entry: string[];
	/** Each page's own modules, by the page's file name. */
	pages: Record<string, string[]>;
}

// The build writes the client's module graph over this placeholder once the
// client is built (`build/preloads.mjs`); a dev server leaves it a string.
// `Reflect.get` keeps the bundler from folding the placeholder away.
const BUILT: unknown = Reflect.get(
	{ value: "@@BENCHER_NEXT_PRELOADS@@" },
	"value",
);

/** Every module `page` loads, so the document can ask for all of them at once. */
export const modulePreloads = (page: PageName): string[] => {
	if (typeof BUILT !== "object" || BUILT === null) {
		return [];
	}
	const { entry, pages } = BUILT as Preloads;
	return [...entry, ...(pages[page] ?? [])];
};
