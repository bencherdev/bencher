import { createMemo, createResource } from "solid-js";
import { isAllowedOrganization } from "../../../util/auth";
import { OrganizationPermission } from "../../../types/bencher";
import bencher_valid_init from "bencher_valid";
import type { Params } from "astro";
import OrgMenuInner from "./OrgMenuInner";

interface Props {
	apiUrl: string;
	params: Params;
}

const OrgMenu = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const params = createMemo(() => props.params);
	const organization = createMemo(() => params().organization);
	const [billing] = createResource(bencher_valid, async (bv) => {
		if (!bv) {
			return false;
		}
		return await isAllowedOrganization(
			props.apiUrl,
			params(),
			OrganizationPermission.Manage,
		);
	});

	return <OrgMenuInner organization={organization} billing={billing} />;
};

export default OrgMenu;
