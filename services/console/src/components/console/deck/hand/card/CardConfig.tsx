import type { Card, Display } from "../../../../../config/types";
import type { Params } from "../../../../../util/url";
import type { PosterFieldConfig } from "../../../poster/Poster";

export interface CardConfig {
	kind: Card;
	label: string;
	key?: string;
	keys?: string[];
	display: Display;
	field: PosterFieldConfig;
	is_allowed: (pathParams: Params) => boolean;
	path: (pathParams: Params, data: Record<string, any>) => string;
}

export default CardConfig;
