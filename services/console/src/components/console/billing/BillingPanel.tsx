import { Show, createMemo } from "solid-js";
import CloudPanel from "./plan/CloudPanel";
import SelfHostedPanel from "./plan/SelfHostedPanel";
import type { Params } from "astro";
import { isBencherCloud } from "../../../util/ext";
import { Host } from "../../../config/organization/billing";
import { Resource } from "../../../config/types";
import type { BillingHeaderConfig } from "./BillingHeader";
import consoleConfig from "../../../config/console";
import NewCloudPanel from "./plan/NewCloudPanel";

interface Props {
	apiUrl: string;
	params: Params;
}

export interface BillingPanelConfig {
	header: BillingHeaderConfig;
	host: Host;
}

const BillingPanel = (props: Props) => {
	const config = createMemo<BillingPanelConfig>(
		() =>
			consoleConfig[Resource.BILLING]?.[
				isBencherCloud() ? Host.BENCHER_CLOUD : Host.SELF_HOSTED
			],
	);

	return (
		<Show
			when={isBencherCloud()}
			fallback={
				<SelfHostedPanel
					apiUrl={props.apiUrl}
					params={props.params}
					config={config}
				/>
			}
		>
			<NewCloudPanel
				apiUrl={props.apiUrl}
				params={props.params}
				config={config}
			/>
		</Show>
	);
};

export default BillingPanel;
