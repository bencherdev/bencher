import type { JsonOrganization } from "../types/bencher";

export const BENCHER_ORGANIZATION_KEY = "BENCHER_ORGANIZATION";

export const setOrganization = (organization: JsonOrganization) => {
	if (typeof localStorage !== "undefined") {
		localStorage.setItem(
			BENCHER_ORGANIZATION_KEY,
			JSON.stringify(organization),
		);
	}
};

export const getOrganization = () => {
	if (typeof localStorage !== "undefined") {
		const organization = localStorage.getItem(BENCHER_ORGANIZATION_KEY);
		if (organization) {
			return JSON.parse(organization);
		}
		clearOrganization();
	}
	return null;
};

export const clearOrganization = () => {
	if (typeof localStorage !== "undefined") {
		localStorage.removeItem(BENCHER_ORGANIZATION_KEY);
	}
};
