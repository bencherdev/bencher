import type { Params } from "astro";
import { useNavigate } from "../../../../util/url";

export interface Props {
	params: Params;
}

const ConsoleButton = (props: Props) => {
	const navigate = useNavigate();

	return (
		<button
			class="button is-outlined is-fullwidth"
			type="button"
			title="View in Console"
			onClick={(e) => {
				e.preventDefault();
				const url = `/console/projects/${props.params?.project}/alerts/${props.params?.alert}`;
				navigate(url);
			}}
		>
			<span class="icon">
				<i class="far fa-window-maximize" aria-hidden="true" />
			</span>
			<span>View in Console</span>
		</button>
	);
};
export default ConsoleButton;
