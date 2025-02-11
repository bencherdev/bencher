import type { Params } from "astro";
import { createMemo, createResource } from "solid-js";
import { OrganizationPermission } from "../../../types/bencher";
import { isAllowedOrganization } from "../../../util/auth";
import { init_valid } from "../../../util/valid";
import OrgMenuInner from "./OrgMenuInner";

interface Props {
	apiUrl: string;
	params: Params;
}

const OrgMenu = (props: Props) => {
	const [bencher_valid] = createResource(init_valid);
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
