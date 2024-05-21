import type { Params } from "astro";
import { BACK_PARAM, encodePath } from "../../../../util/url";

export interface Props {
	params: Params;
	resource: string;
	param: string;
}

const ConsoleButton = (props: Props) => {
	return (
		<a
			class="button is-fullwidth"
			type="button"
			title="View in Console"
			href={`/console/projects/${props.params?.project}/${props.resource}/${
				props.params?.[props.param]
			}?${BACK_PARAM}=${encodePath()}`}
		>
			<span class="icon">
				<i class="far fa-window-maximize" />
			</span>
			<span>View in Console</span>
		</a>
	);
};
export default ConsoleButton;
