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

const docs = (section: string) =>
	defineCollection({
		loader: glob({ pattern: I18N_MDX, base: `${CONTENT}/docs-${section}` }),
		schema: pageSchema,
	});

const api = defineCollection({
	loader: glob({ pattern: "api-*", base: CONTENT }),
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

const benchmarking = defineCollection({
	loader: glob({ pattern: "benchmarking-*.mdx", base: CONTENT }),
	schema: pageSchema,
});

const track_in_ci = defineCollection({
	loader: glob({ pattern: "track-in-ci-*.mdx", base: CONTENT }),
	schema: pageSchema,
});

const case_study = defineCollection({
	loader: glob({ pattern: "case-study.mdx", base: CONTENT }),
	schema: pageSchema,
});

const engineering = defineCollection({
	loader: glob({ pattern: "engineering.mdx", base: CONTENT }),
	schema: pageSchema,
});

const onboard = defineCollection({
	loader: glob({ pattern: "onboard.mdx", base: CONTENT }),
	schema: pageSchema,
});

// 3. Export a single `collections` object to register your collection(s)
export const collections = {
	// legal
	legal: legal,
	// docs
	docs_tutorial: docs("tutorial"),
	docs_how_to: docs,
	docs_explanation: docs,
	docs_reference: docs,
	// api
	api_organizations: api,
	api_projects: api,
	api_users: api,
	api_server: api,
	// learn
	benchmarking_cpp: benchmarking,
	benchmarking_python: benchmarking,
	benchmarking_rust: benchmarking,
	track_in_ci_cpp: track_in_ci,
	track_in_ci_python: track_in_ci,
	track_in_ci_rust: track_in_ci,
	case_study: case_study,
	engineering: engineering,
	// onboard
	onboard: onboard,
};

export const keywords = ["Continuous Benchmarking"];
