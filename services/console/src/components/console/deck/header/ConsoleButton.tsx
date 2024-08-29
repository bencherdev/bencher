import type { Params } from "astro";
import { BACK_PARAM, encodePath, useSearchParams } from "../../../../util/url";
import { PubResource, type PubResourceKind } from "../../../perf/util";
import { createMemo } from "solid-js";

export interface Props {
	params: Params;
	resource: PubResourceKind;
}

const ConsoleButton = (props: Props) => {
	const [searchParams, _setSearchParams] = useSearchParams();

	const queryString = createMemo(() => {
		const newSearchParams = new URLSearchParams();
		const params = PubResource.search(props.resource, searchParams);
		for (const [key, value] of Object.entries(params)) {
			if (value) {
				newSearchParams.set(key, value.toString());
			}
		}
		newSearchParams.set(BACK_PARAM, encodePath());
		return newSearchParams.toString();
	});

	return (
		<a
			class="button is-fullwidth"
			type="button"
			title="View in Console"
			href={`/console/projects/${props.params?.project}/${PubResource.resource(
				props.resource,
			)}/${props.params?.[PubResource.param(props.resource)]}?${queryString()}`}
		>
			<span class="icon">
				<i class="far fa-window-maximize" />
			</span>
			<span>View in Console</span>
		</a>
	);
};
export default ConsoleButton;
