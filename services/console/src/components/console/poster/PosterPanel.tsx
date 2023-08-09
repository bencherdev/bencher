import { createMemo } from "solid-js";
import { Operation, type Resource } from "../../../config/types";
import Poster, { PosterConfig } from "./Poster";
import PosterHeader, { PosterHeaderConfig } from "./PosterHeader";
import consoleConfig from "../../../config/console";
import type { Params } from "astro";

interface Props {
	params: Params;
	resource: Resource;
}

interface PosterPanelConfig {
	operation: Operation;
	header: PosterHeaderConfig;
	form: PosterConfig;
}

const PosterPanel = (props: Props) => {
	const config = createMemo<PosterPanelConfig>(
		() => consoleConfig[props.resource]?.[Operation.ADD],
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
