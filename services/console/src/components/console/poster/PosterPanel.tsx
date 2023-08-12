import { createMemo } from "solid-js";
import { Operation, type Resource } from "../../../config/types";
import Poster, { PosterConfig } from "./Poster";
import PosterHeader, { PosterHeaderConfig } from "./PosterHeader";
import consoleConfig from "../../../config/console";
import type { Params } from "astro";

interface Props {
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
				params={props.params}
				operation={config()?.operation}
				config={config()?.form}
			/>
		</>
	);
};

export default PosterPanel;
