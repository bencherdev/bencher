import { createMemo } from "solid-js";
import { Operation, type Resource } from "../../../config/types";
import Poster, { type PosterConfig } from "./Poster";
import PosterHeader, { type PosterHeaderConfig } from "./PosterHeader";
import consoleConfig from "../../../config/console";
import type { Params } from "astro";

interface Props {
	apiUrl: string;
	params: Params;
	resource: Resource;
	operation?: Operation;
}

interface PosterPanelConfig {
	operation: Operation;
	header: PosterHeaderConfig;
	form: PosterConfig;
}

const PosterPanel = (props: Props) => {
	const operation = createMemo(
		() => props.operation || Operation.ADD,
		Operation.ADD,
	);
	const config = createMemo<PosterPanelConfig>(
		() => consoleConfig[props.resource]?.[operation()],
	);

	return (
		<>
			<PosterHeader config={config()?.header} />
			<Poster
				apiUrl={props.apiUrl}
				params={props.params}
				resource={props.resource}
				operation={config()?.operation}
				config={config()?.form}
			/>
		</>
	);
};

export default PosterPanel;
