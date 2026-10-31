import { useQueryClient } from "@tanstack/solid-query";
import { For } from "solid-js";
import type { Api } from "../api";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import { Clear, FilterMenu, type Listed } from "./Controls";
import { totalOf } from "../list/pages";

export type Dimension = "branches" | "testbeds" | "measures";
type Filter = "branch" | "testbed" | "measure";

/** A UUID or a name for each filter, by dimension. */
export type FilterNames = { [K in Filter]?: string | undefined };

const RESOURCES: Record<Filter, Dimension> = {
	branch: "branches",
	testbed: "testbeds",
	measure: "measures",
};

/** How many options a filter menu lists at once; search finds the rest. */
const OPTIONS = 32;

/** The name of a branch, a testbed, or a measure, by UUID. */
const nameKey = (resource: Dimension, slug: string, uuid: string) =>
	["console", resource, slug, "name", uuid] as const;

export const nameQuery = (
	api: Api,
	slug: string,
	resource: Dimension,
	uuid: string | undefined,
) => ({
	queryKey: nameKey(resource, slug, uuid ?? ""),
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(
			await api.get<{ name: string }>(
				`/v0/projects/${encodeURIComponent(slug)}/${resource}/${encodeURIComponent(uuid ?? "")}`,
				signal,
			)
		).data.name,
	enabled: uuid !== undefined,
});

/** A menu each to filter by branch, testbed, and measure by UUID, its options and chip by name, and Clear. */
const DimensionFilters = (props: {
	/** The UUIDs picked. */
	values: FilterNames;
	names: FilterNames;
	/** Offer archived ones too, since archiving one archives its thresholds. */
	archived?: boolean;
	onPick: (filter: Filter, uuid: string | undefined) => void;
	clearable: boolean;
	onClear: () => void;
}) => {
	const { slug } = useProject();
	const client = useQueryClient();
	return (
		<>
			<For each={["branch", "testbed", "measure"] as const}>
				{(filter) => (
					<FilterMenu
						name={filter}
						value={props.values[filter]}
						// A cold link knows the UUID before the name.
						label={
							props.names[filter] ??
							(props.values[filter] === undefined ? undefined : "…")
						}
						search
						options={(text) =>
							useOptions(RESOURCES[filter], text, () => props.archived === true)
						}
						onPick={(uuid, label) => {
							// The chip names the pick without asking the API again.
							if (uuid !== undefined && label !== undefined) {
								client.setQueryData(
									nameKey(RESOURCES[filter], slug(), uuid),
									label,
								);
							}
							props.onPick(filter, uuid);
						}}
					/>
				)}
			</For>
			<Clear when={props.clearable} onClear={props.onClear} />
		</>
	);
};

export default DimensionFilters;

/**
 * A project's branches, testbeds, or measures whose name or slug matches
 * `text`, by UUID; with `archived`, the archived ones too, in name order.
 */
const useOptions = (
	resource: Dimension,
	text: () => string,
	archived: () => boolean,
) => {
	const active = useListed(resource, text, false, () => true);
	const retired = useListed(resource, text, true, archived);
	return (): Listed => {
		const lists = archived() ? [active(), retired()] : [active()];
		const rows = lists.flatMap((list) => list.rows);
		return {
			options: (archived() ? rows.sort(byName) : rows).map(
				({ uuid, name }) => ({ value: uuid, label: name }),
			),
			more: lists.reduce(
				(sum, { rows, total }) => sum + Math.max(0, total - rows.length),
				0,
			),
		};
	};
};

/** One page of a project's active or archived branches, testbeds, or measures. */
const useListed = (
	resource: Dimension,
	text: () => string,
	archived: boolean,
	enabled: () => boolean,
) => {
	const { api, slug } = useProject();
	const result = useQueryResult(() => ({
		queryKey: archived
			? ["console", resource, slug(), text(), "archived"]
			: ["console", resource, slug(), text()],
		enabled: enabled(),
		// The last options stay while the next search loads, so the list never blinks empty.
		placeholderData: (previous: Matched | undefined) => previous,
		queryFn: async ({ signal }: { signal: AbortSignal }): Promise<Matched> => {
			const params = new URLSearchParams({ per_page: String(OPTIONS) });
			if (text()) {
				params.set("search", text());
			}
			if (archived) {
				params.set("archived", "true");
			}
			const { data, headers } = await api.get<Named[]>(
				`/v0/projects/${encodeURIComponent(slug())}/${resource}?${params}`,
				signal,
			);
			return { rows: data, total: totalOf(headers, data) };
		},
	}));
	return () => result().data ?? { rows: [], total: 0 };
};

const byName = (a: Named, b: Named) =>
	a.name < b.name ? -1 : a.name > b.name ? 1 : 0;

interface Named {
	uuid: string;
	name: string;
}

interface Matched {
	rows: Named[];
	/** Every one the search matches. */
	total: number;
}
