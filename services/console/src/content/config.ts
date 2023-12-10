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
	}),
});
const how_to = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
	}),
});
const explanation = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
		canonical: z.string().optional(),
	}),
});
const reference = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
		canonical: z.string().optional(),
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
	// learn
	rust: rust,
};

export const keywords = ["Continuous Benchmarking"];
