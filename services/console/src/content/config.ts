// https://docs.astro.build/en/guides/content-collections/
// 1. Import utilities from `astro:content`
import { defineCollection, z } from "astro:content";

export enum Collection {
	// Legal
	legal = "legal",
	// Docs
	tutorial = "tutorial",
	how_to = "how_to",
	explanation = "explanation",
	reference = "reference",
	// API
	organizations = "organizations",
	// Learn
	rust = "rust",
}

// 2. Define a `type` and `schema` for each collection
const legal = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
		draft: z.boolean().optional(),
	}),
});

// docs
const tutorial = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
		draft: z.boolean().optional(),
	}),
});
const how_to = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
		draft: z.boolean().optional(),
	}),
});
const explanation = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
		draft: z.boolean().optional(),
		canonicalize: z.boolean().optional(),
	}),
});
const reference = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
		draft: z.boolean().optional(),
		canonicalize: z.boolean().optional(),
	}),
});

// api
const organizations = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
		draft: z.boolean().optional(),
		paths: z.array(
			z.object({ path: z.string(), method: z.string(), cli: z.string() }),
		),
	}),
});

// learn
const rust = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
		draft: z.boolean().optional(),
	}),
});

// 3. Export a single `collections` object to register your collection(s)
export const collections = {
	legal: legal,
	// docs
	tutorial: tutorial,
	how_to: how_to,
	explanation: explanation,
	reference: reference,
	// api
	organizations: organizations,
	// learn
	rust: rust,
};

export const keywords = ["Continuous Benchmarking"];
