export const fmtValues = (
	data: Record<string, any>,
	key: undefined | string,
	keys: undefined | string[][],
	separator: string,
): undefined | number | string => {
	if (!data) {
		return;
	} else if (key) {
		return data[key];
	} else if (keys) {
		return keys.reduce(
			(accumulator: string, current: string[], index: number) => {
				const value = fmtNestedValue(data, current);
				if (index === 0) {
					return value ? value : "";
				} else {
					return accumulator + separator + value;
				}
			},
			"",
		);
	} else {
		return "Unknown Item";
	}
};

export const fmtNestedValue = (
	datum: Record<string, any>,
	keys: string[],
): undefined | string => {
	if (!datum) {
		return;
	}
	return keys
		.reduce(
			(
				accumulator: Record<string, any>,
				current: number | string,
				index: number,
			) => {
				if (index === 0) {
					return datum[current];
				} else {
					return accumulator?.[current];
				}
			},
			{},
		)
		.toString();
};

const BENCHER_TITLE = "Bencher - Continuous Benchmarking";
export const fmtPageTitle = (title: undefined | string) =>
	title ? `${title} | ${BENCHER_TITLE}` : BENCHER_TITLE;

export const setPageTitle = (title: undefined | string) => {
	const page_title = fmtPageTitle(title);
	if (document.title === page_title) {
		return;
	} else {
		document.title = page_title;
	}
};
