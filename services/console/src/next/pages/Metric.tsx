import { useParams } from "@solidjs/router";
import { onCleanup, onMount } from "solid-js";
import type { JsonOneMetric } from "../../types/bencher";
import ContentSkeleton from "../ContentSkeleton";
import { useProject, useProjectNavigate } from "../project";

/** A metric has no page: its link opens Explore on its line, or on nothing when it cannot be read. */
const Metric = () => {
	const { api, slug } = useProject();
	const params = useParams();
	const navigate = useProjectNavigate();
	const reading = new AbortController();
	onCleanup(() => reading.abort());
	onMount(async () => {
		// Explore's query codec loads beside the read; reading only its export spares the bundler a namespace object.
		const codec = import("../metric/explore").then(({ exploreSearchOf }) => ({
			exploreSearchOf,
		}));
		const search = await Promise.all([
			api.get<JsonOneMetric>(
				`/v0/projects/${encodeURIComponent(slug())}/metrics/${encodeURIComponent(params.metric ?? "")}`,
				reading.signal,
			),
			codec,
		]).then(
			([{ data }, { exploreSearchOf }]) => exploreSearchOf(data),
			() => "",
		);
		if (!reading.signal.aborted) {
			navigate(`/${slug()}/explore${search}`, { replace: true });
		}
	});
	return <ContentSkeleton />;
};

export default Metric;
