import { defineCollection, z } from "astro:content";
import { glob } from "astro/loaders";

const MDX = "*.mdx";
const I18N_MDX = `**/${MDX}`;
const CONTENT = "./src/content";

const pageSchema = z.object({
	title: z.string(),
	description: z.string(),
	heading: z.string(),
	published: z.string().optional(),
	modified: z.string().optional(),
	sortOrder: z.number(),
	draft: z.boolean().optional(),
	canonicalize: z.boolean().optional(),
});

const legal = defineCollection({
	loader: glob({ pattern: MDX, base: `${CONTENT}/legal` }),
	schema: pageSchema,
});

const i18n_collection = (collection: string) =>
	defineCollection({
		loader: glob({ pattern: I18N_MDX, base: `${CONTENT}/${collection}` }),
		schema: pageSchema,
	});

const docs = (section: string) => i18n_collection(`docs-${section}`);

const api = (resource: string) =>
	defineCollection({
		loader: glob({ pattern: MDX, base: `${CONTENT}/api-${resource}` }),
		schema: z.object({
			title: z.string(),
			description: z.string(),
			heading: z.string(),
			published: z.string().optional(),
			modified: z.string().optional(),
			sortOrder: z.number(),
			draft: z.boolean().optional(),
			canonicalize: z.boolean().optional(),
			paths: z.array(
				z.object({
					path: z.string(),
					method: z.string(),
					headers: z.string(),
					cli: z.string().optional().nullable(),
				}),
			),
		}),
	});

enum Lang {
	Cpp = "cpp",
	Python = "python",
	Rust = "rust",
}

const benchmarking = (lang: Lang) => learn("benchmarking", lang);

const track_in_ci = (lang: Lang) => learn("track-in-ci", lang);

const learn = (section: string, lang: Lang) =>
	i18n_collection(`${section}-${lang}`);

export const collections = {
	// legal
	legal: legal,
	// docs
	docs_tutorial: docs("tutorial"),
	docs_how_to: docs("how-to"),
	docs_explanation: docs("explanation"),
	docs_reference: docs("reference"),
	// api
	api_organizations: api("organizations"),
	api_projects: api("projects"),
	api_users: api("users"),
	api_server: api("server"),
	// learn
	benchmarking_cpp: benchmarking(Lang.Cpp),
	benchmarking_python: benchmarking(Lang.Python),
	benchmarking_rust: benchmarking(Lang.Rust),
	track_in_ci_cpp: track_in_ci(Lang.Cpp),
	track_in_ci_python: track_in_ci(Lang.Python),
	track_in_ci_rust: track_in_ci(Lang.Rust),
	case_study: i18n_collection("case-study"),
	engineering: i18n_collection("engineering"),
	// onboard
	onboard: i18n_collection("onboard"),
};

export const keywords = ["Continuous Benchmarking"];
