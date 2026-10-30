import type { JsonThreshold, JsonVariant } from "../../types/bencher";
import type { Api } from "../api";
import { createPaged } from "../list/paged";
import {
	THRESHOLDS_PAGE,
	VARIANTS_PAGE,
	thresholdsKey,
	thresholdsPage,
	variantsPage,
	variantsPrefix,
} from "./data";
import type { Dimension } from "./dimension";

/** The thresholds on a branch, testbed, or measure that share its archived state, a page at a time. */
export const thresholdList = (
	api: Api,
	on: () => {
		slug: string;
		dimension: Dimension;
		ref: string;
		archived: boolean;
	},
	enabled: () => boolean,
) =>
	createPaged<JsonThreshold>({
		queryKey: () => {
			const { slug, dimension, ref, archived } = on();
			return thresholdsKey(slug, dimension, ref, archived);
		},
		perPage: THRESHOLDS_PAGE,
		fetchPage: (page, perPage, signal) => {
			const { slug, dimension, ref, archived } = on();
			return thresholdsPage(
				api,
				slug,
				dimension,
				ref,
				archived,
			)(page, perPage, signal);
		},
		enabled,
		keep: true,
	});

/** A benchmark's variants with one archived state, a page at a time, or with `drain` all at once. */
export const variantList = (
	api: Api,
	on: () => { slug: string; ref: string; archived: boolean },
	drain = false,
) =>
	createPaged<JsonVariant>({
		queryKey: () => {
			const { slug, ref, archived } = on();
			return [...variantsPrefix(slug, ref), archived];
		},
		perPage: VARIANTS_PAGE,
		fetchPage: (page, perPage, signal) => {
			const { slug, ref, archived } = on();
			return variantsPage(api, slug, ref, archived)(page, perPage, signal);
		},
		keep: true,
		drain,
	});
