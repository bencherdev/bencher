export const fmtValues = (
	data: undefined | Record<string, any>,
	key: undefined | string,
	keys: undefined | string[][],
	separator: string,
): undefined | number | string => {
	if (!data) {
		return;
	} else if (key) {
		return data[key];
	} else if (keys) {
		return keys.reduce((accumulator, current, index) => {
			const value = fmtNestedValue(data, current);
			if (index === 0) {
				return value;
			} else {
				return accumulator + separator + value;
			}
		}, "");
	} else {
		return "Unknown Item";
	}
};

export const fmtNestedValue = (
	datum: undefined | Record<string, any>,
	keys: undefined | string[],
): string => {
	if (!datum) {
		return "";
	}
	return (
		keys?.reduce((accumulator, current, index) => {
			if (index === 0) {
				return datum[current];
			} else {
				return accumulator?.[current];
			}
		}, "") ?? ""
	);
};

export const BENCHER_TITLE = "Bencher - Continuous Benchmarking";
export const BENCHER_DESCRIPTION =
	"Catch performance regressions in CI with continuous benchmarking";

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
