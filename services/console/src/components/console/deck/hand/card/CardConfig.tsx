import type { Params } from "astro";
import type { Card, Display } from "../../../../../config/types";
import type { PosterFieldConfig } from "../../../poster/Poster";

export interface CardConfig {
	kind: Card;
	label: string;
	key?: string;
	keys?: string[];
	icon?: string;
	display: Display;
	field: PosterFieldConfig;
	is_allowed: (
		apiUrl: string,
		params: Params,
		isBencherCloud?: boolean,
	) => boolean;
	path: (params: Params, data: object) => string;
	notify?: boolean;
	/** What to tell the user when an update waits on something else, such as a confirm link. */
	pending?: (data: object, response: object) => undefined | string;
}

export default CardConfig;
