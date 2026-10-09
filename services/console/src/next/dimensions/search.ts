export type Sort = "name" | "created" | "last_used";
export type Direction = "asc" | "desc";

/** A dimension list's view, as its link carries it. */
export interface ListSearch {
	archived: boolean;
	sort: Sort;
	direction: Direction;
	/** A name, slug, or UUID the rows must contain. */
	search: string;
}

/** One batch of the console's list: `perPage` rows after the first `offset`. */
export interface Batch {
	offset: number;
	perPage: number;
}

const SORTS: readonly Sort[] = ["name", "created", "last_used"];

export const DEFAULT_SEARCH: ListSearch = {
	archived: false,
	sort: "name",
	direction: "asc",
	search: "",
};

export const decodeSearch = (params: URLSearchParams): ListSearch => {
	const sort = SORTS.find((known) => known === params.get("sort")) ?? "name";
	const direction = params.get("direction");
	return {
		archived: params.get("archived") === "true",
		sort,
		direction:
			direction === "asc" || direction === "desc"
				? direction
				: defaultDirection(sort),
		search: params.get("search")?.trim() ?? "",
	};
};

export const encodeSearch = (search: ListSearch) => {
	const params = new URLSearchParams();
	if (search.archived) {
		params.set("archived", "true");
	}
	if (search.sort !== DEFAULT_SEARCH.sort) {
		params.set("sort", search.sort);
	}
	if (search.direction !== defaultDirection(search.sort)) {
		params.set("direction", search.direction);
	}
	if (search.search) {
		params.set("search", search.search);
	}
	return params.toString();
};

export const pickSort = (search: ListSearch, sort: Sort): ListSearch => ({
	...search,
	sort,
	direction: defaultDirection(sort),
});

export const flipDirection = (search: ListSearch): ListSearch => ({
	...search,
	direction: search.direction === "asc" ? "desc" : "asc",
});

export const directionText = (sort: Sort, direction: Direction) => {
	if (sort === "name") {
		return direction === "asc" ? "A to Z" : "Z to A";
	}
	return direction === "desc" ? "Newest first" : "Oldest first";
};

/** The query for one batch of the console's list. */
export const apiParams = (search: ListSearch, batch: Batch) => {
	const params = new URLSearchParams();
	if (search.archived) {
		params.set("archived", "true");
	}
	params.set("sort", search.sort);
	params.set("direction", search.direction);
	if (search.search) {
		params.set("search", search.search);
	}
	params.set("offset", String(batch.offset));
	params.set("per_page", String(batch.perPage));
	return params.toString();
};

/** Names read A to Z; times newest first. */
const defaultDirection = (sort: Sort): Direction =>
	sort === "name" ? "asc" : "desc";
