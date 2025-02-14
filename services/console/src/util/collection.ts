import { Language } from "../i18n/ui";

enum Collection {
	// Legal
	legal = "legal",
	// Docs
	docs_tutorial = "docs-tutorial",
	docs_how_to = "docs-how-to",
	docs_explanation = "docs-explanation",
	docs_reference = "docs-reference",
	// API
	api_organizations = "api-organizations",
	api_projects = "api-projects",
	api_users = "api-users",
	api_server = "api-server",
	// Learn
	benchmarking_cpp = "benchmarking-cpp",
	benchmarking_python = "benchmarking-python",
	benchmarking_rust = "benchmarking-rust",
	track_in_ci_cpp = "track-in-ci-cpp",
	track_in_ci_python = "track-in-ci-python",
	track_in_ci_rust = "track-in-ci-rust",
	case_study = "case-study",
	engineering = "engineering",
	// Onboard,
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

export const fmtPageId = (page_id: string) =>
	page_id
		// Remove the language prefix from the page_id (`en/`)
		.replace(new RegExp(`^(${Object.values(Language).join("|")})/`), "")
		// Remove the file extension from the page_id (`.mdx`)
		.replace(/\.mdx$/, "");

export default Collection;
