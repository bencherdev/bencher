import { useLocation } from "@solidjs/router";
import Heading from "@bencherdev/ui/Heading";
import { useQueryClient } from "@tanstack/solid-query";
import { Show, createComputed, createMemo } from "solid-js";
import ContentSkeleton from "../ContentSkeleton";
import { listQuery, screenBatch } from "../dimensions/data";
import { KINDS, placeOf } from "../dimensions/dimension";
import Inspect, { prefetchInspect } from "../dimensions/Inspect";
import List from "../dimensions/List";
import { decodeSearch } from "../dimensions/search";
import { parseNextPath } from "../paths";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import Frame from "../settings/Frame";
import Head from "../settings/Head";

/** Settings' Dimensions: a list of each dimension, and each one's own page. */
const Dimensions = () => {
	const location = useLocation();
	const { api, slug } = useProject();
	const client = useQueryClient();
	const place = createMemo(
		() => placeOf(parseNextPath(location.pathname)?.rest ?? []),
		undefined,
		{
			equals: (a, b) => a?.dimension === b?.dimension && a?.entry === b?.entry,
		},
	);
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	const perPage = screenBatch();
	// A page paints once the shell says what the reader may do, so its own
	// requests start beside the shell's rather than a round after it.
	createComputed(() => {
		const at = place();
		if (!at || bootstrap().data) {
			return;
		}
		if (at.entry === undefined) {
			client.prefetchQuery(
				listQuery(
					api,
					slug(),
					at.dimension,
					decodeSearch(new URLSearchParams(location.search)),
					{ ordinal: 1, perPage },
				),
			);
		} else {
			prefetchInspect(client, api, slug(), at.dimension, at.entry);
		}
	});
	return (
		<Show when={bootstrap().data} fallback={<ContentSkeleton />}>
			{(shown) => (
				<Frame slug={slug()} section="dimensions">
					<Show when={place()} keyed>
						{(at) =>
							at.entry === undefined ? (
								<>
									<Head
										slug={slug()}
										title={KINDS[at.dimension].label}
										readOnly={!shown().permissions.edit}
										crumbs={[{ label: "Dimensions", path: "branches" }]}
									>
										<p class="set-lede">{KINDS[at.dimension].lede}</p>
									</Head>
									<List
										dimension={at.dimension}
										perPage={perPage}
										edit={shown().permissions.edit}
									/>
								</>
							) : (
								<Inspect
									dimension={at.dimension}
									entry={at.entry}
									edit={shown().permissions.edit}
								/>
							)
						}
					</Show>
					<Show when={!place()}>
						<Heading level={1} size="xl">
							Page not found
						</Heading>
					</Show>
				</Frame>
			)}
		</Show>
	);
};

export default Dimensions;
