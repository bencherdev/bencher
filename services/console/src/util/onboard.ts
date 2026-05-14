const BENCHER_ONBOARD_PROJECT_KEY = "BENCHER_ONBOARD_PROJECT_KEY";

export const setOnboardProjectKey = (key: string) => {
	if (typeof sessionStorage !== "undefined") {
		sessionStorage.setItem(BENCHER_ONBOARD_PROJECT_KEY, key);
	}
};

export const getOnboardProjectKey = (): string | null => {
	if (typeof sessionStorage !== "undefined") {
		return sessionStorage.getItem(BENCHER_ONBOARD_PROJECT_KEY);
	}
	return null;
};

export const removeOnboardProjectKey = () => {
	if (typeof sessionStorage !== "undefined") {
		sessionStorage.removeItem(BENCHER_ONBOARD_PROJECT_KEY);
	}
};
