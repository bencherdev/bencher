// Type declarations for the Pagefind runtime module served at /pagefind/pagefind.js.
// Pagefind is loaded via a runtime dynamic import and is not present in node_modules.
// See https://pagefind.app/docs/api/ for the upstream API reference.

export interface PagefindSubResult {
	title: string;
	url: string;
	excerpt: string;
	anchor?: {
		element: string;
		id: string;
		text?: string;
		location: number;
	};
}

export interface PagefindResultData {
	url: string;
	raw_url?: string;
	content: string;
	excerpt: string;
	word_count: number;
	filters: Record<string, string[]>;
	meta: {
		title?: string;
		image?: string;
		image_alt?: string;
		[key: string]: string | undefined;
	};
	sub_results: PagefindSubResult[];
}

export interface PagefindResult {
	id: string;
	data: () => Promise<PagefindResultData>;
}

export interface PagefindSearchResponse {
	results: PagefindResult[];
	unfilteredResultCount: number;
	filters: Record<string, Record<string, number>>;
	totalFilters: Record<string, Record<string, number>>;
	timings: {
		preload: number;
		search: number;
		total: number;
	};
}

export interface PagefindSearchOptions {
	filters?: Record<string, string[] | string>;
	sort?: Record<string, "asc" | "desc">;
}

export interface PagefindApi {
	search: (
		query: string,
		options?: PagefindSearchOptions,
	) => Promise<PagefindSearchResponse>;
	options: (opts: Record<string, unknown>) => Promise<void>;
	init?: () => Promise<void>;
}
