import type { Params } from "astro";
import type { Card, Display } from "../../../../../config/types";
import type { PosterFieldConfig } from "../../../poster/Poster";

export interface CardConfig {
	kind: Card;
	label: string;
	key?: string;
	keys?: string[];
	display: Display;
	field: PosterFieldConfig;
	is_allowed: (apiUrl: string, params: Params) => boolean;
	path: (params: Params, data: object) => string;
}

export default CardConfig;
