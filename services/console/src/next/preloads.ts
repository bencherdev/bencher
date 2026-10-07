import type { Tab } from "./paths";

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

const PAGE_FILES: Record<Tab, string> = {
	explore: "Explore",
	plots: "Plots",
	reports: "Reports",
	alerts: "Alerts",
	thresholds: "Thresholds",
	settings: "Settings",
};

/** Every module the page for `tab` loads, so the document can ask for all of them at once. */
export const modulePreloads = (tab: Tab | undefined): string[] => {
	if (typeof BUILT !== "object" || BUILT === null) {
		return [];
	}
	const { entry, pages } = BUILT as Preloads;
	return [...entry, ...(pages[tab ? PAGE_FILES[tab] : "NotFound"] ?? [])];
};
