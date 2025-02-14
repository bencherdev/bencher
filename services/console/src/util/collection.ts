import { Language } from "../i18n/ui";

enum Collection {
	// Legal
	legal = "legal",
	// Docs
	docs_tutorial = "docs_tutorial",
	docs_how_to = "docs_how_to",
	docs_explanation = "docs_explanation",
	docs_reference = "docs_reference",
	// API
	api_organizations = "api_organizations",
	api_projects = "api_projects",
	api_users = "api_users",
	api_server = "api_server",
	// Learn
	benchmarking_cpp = "benchmarking_cpp",
	benchmarking_python = "benchmarking_python",
	benchmarking_rust = "benchmarking_rust",
	track_in_ci_cpp = "track_in_ci_cpp",
	track_in_ci_python = "track_in_ci_python",
	track_in_ci_rust = "track_in_ci_rust",
	case_study = "case_study",
	engineering = "engineering",
	// Onboard
	onboard = "onboard",
}

export const ApiCollections = [
	Collection.api_organizations,
	Collection.api_projects,
	Collection.api_users,
	Collection.api_server,
];

export const collectionPath = (collection: Collection) => {
	switch (collection) {
		case Collection.legal:
			return "legal";
		case Collection.docs_tutorial:
			return "tutorial";
		case Collection.docs_how_to:
			return "how-to";
		case Collection.docs_explanation:
			return "explanation";
		case Collection.docs_reference:
			return "reference";
		case Collection.api_organizations:
			return "organizations";
		case Collection.api_projects:
			return "projects";
		case Collection.api_users:
			return "users";
		case Collection.api_server:
			return "server";
		case Collection.benchmarking_cpp:
			return "benchmarking/cpp";
		case Collection.benchmarking_python:
			return "benchmarking/python";
		case Collection.benchmarking_rust:
			return "benchmarking/rust";
		case Collection.track_in_ci_cpp:
			return "track-in-ci/cpp";
		case Collection.track_in_ci_python:
			return "track-in-ci/python";
		case Collection.track_in_ci_rust:
			return "track-in-ci/rust";
		case Collection.case_study:
			return "case-study";
		case Collection.engineering:
			return "engineering";
		case Collection.onboard:
			return "onboard";
	}
};

export const splitPageId = (page_id: string): [undefined | string, string] => {
	const langPattern = `^(${Object.values(Language).join("|")})/`;
	const lang = page_id.match(new RegExp(langPattern))?.[1];
	if (lang) {
		const slug = page_id
			.replace(new RegExp(langPattern), "")
			.replace(/\.mdx$/, "");
		return [lang, slug];
	}
	const slug = page_id.replace(/\.mdx$/, "");
	return [lang, slug];
};

export const fmtPageId = (page_id: string) => splitPageId(page_id)[1];

export default Collection;
