import { describe, expect, test } from "vitest";
import Collection, {
	collectionPath,
	fmtPageId,
	splitPageId,
} from "./collection";

describe("collectionPath", () => {
	const cases: [Collection, string][] = [
		[Collection.legal, "legal"],
		[Collection.docs_tutorial, "tutorial"],
		[Collection.docs_how_to, "how-to"],
		[Collection.docs_explanation, "explanation"],
		[Collection.docs_reference, "reference"],
		[Collection.api_run, "run"],
		[Collection.api_organizations, "organizations"],
		[Collection.api_projects, "projects"],
		[Collection.api_users, "users"],
		[Collection.api_server, "server"],
		[Collection.benchmarking_cpp, "benchmarking/cpp"],
		[Collection.benchmarking_python, "benchmarking/python"],
		[Collection.benchmarking_rust, "benchmarking/rust"],
		[Collection.track_in_ci_cpp, "track-in-ci/cpp"],
		[Collection.track_in_ci_python, "track-in-ci/python"],
		[Collection.track_in_ci_rust, "track-in-ci/rust"],
		[Collection.case_study, "case-study"],
		[Collection.engineering, "engineering"],
		[Collection.onboard, "onboard"],
	];

	test.each(cases)("%s -> %s", (collection, expected) => {
		expect(collectionPath(collection)).toBe(expected);
	});
});

describe("splitPageId", () => {
	test("splits language prefix and slug", () => {
		expect(splitPageId("en/getting-started")).toEqual([
			"en",
			"getting-started",
		]);
	});

	test("handles page without language prefix", () => {
		const [lang, slug] = splitPageId("getting-started");
		expect(lang).toBeUndefined();
		expect(slug).toBe("getting-started");
	});

	test("strips .mdx extension", () => {
		expect(splitPageId("en/page.mdx")).toEqual(["en", "page"]);
	});

	test("strips .mdx extension without language", () => {
		const [lang, slug] = splitPageId("page.mdx");
		expect(lang).toBeUndefined();
		expect(slug).toBe("page");
	});

	test("handles all supported languages", () => {
		for (const lang of ["de", "es", "fr", "ja", "ko", "pt", "ru", "zh"]) {
			const [parsedLang, slug] = splitPageId(`${lang}/page`);
			expect(parsedLang).toBe(lang);
			expect(slug).toBe("page");
		}
	});
});

describe("fmtPageId", () => {
	test("returns slug without language prefix", () => {
		expect(fmtPageId("en/getting-started")).toBe("getting-started");
	});

	test("returns slug when no language prefix", () => {
		expect(fmtPageId("getting-started")).toBe("getting-started");
	});
});
