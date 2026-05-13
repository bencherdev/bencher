const BENCHER_ONBOARD_PROJECT_KEY = "BENCHER_ONBOARD_PROJECT_KEY";

export const setOnboardProjectKey = (key: string) => {
	if (typeof localStorage !== "undefined") {
		localStorage.setItem(BENCHER_ONBOARD_PROJECT_KEY, key);
	}
};

export const getOnboardProjectKey = (): string | null => {
	if (typeof localStorage !== "undefined") {
		return localStorage.getItem(BENCHER_ONBOARD_PROJECT_KEY);
	}
	return null;
};

export const removeOnboardProjectKey = () => {
	if (typeof localStorage !== "undefined") {
		localStorage.removeItem(BENCHER_ONBOARD_PROJECT_KEY);
	}
};
