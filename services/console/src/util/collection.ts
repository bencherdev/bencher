enum Collection {
	// Legal
	legal = "legal",
	// Docs
	tutorial = "tutorial",
	how_to = "how_to",
	explanation = "explanation",
	reference = "reference",
	// API
	organizations = "organizations",
	projects = "projects",
	users = "users",
	server = "server",
	// Learn
	rust = "rust",
}

export const ApiCollections = [Collection.organizations];

export const collectionPath = (collection: Collection) => {
	switch (collection) {
		case Collection.legal:
			return "legal";
		case Collection.tutorial:
			return "tutorial";
		case Collection.how_to:
			return "how-to";
		case Collection.explanation:
			return "explanation";
		case Collection.reference:
			return "reference";
		case Collection.organizations:
			return "organizations";
		case Collection.projects:
			return "projects";
		case Collection.users:
			return "users";
		case Collection.server:
			return "server";
		case Collection.rust:
			return "benchmarking/rust";
	}
};

export default Collection;
