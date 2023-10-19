// 1. Import utilities from `astro:content`
import { z, defineCollection } from "astro:content";

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
	}),
});
const reference = defineCollection({
	type: "content", // v2.5.0 and later
	schema: z.object({
		title: z.string(),
		description: z.string(),
		heading: z.string(),
		sortOrder: z.number(),
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
