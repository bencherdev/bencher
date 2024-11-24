import { type JsonPlot, XAxis } from "../../types/bencher";
import { defaultUser } from "../../util/auth";
import { BENCHER_CLOUD_API_URL } from "../../util/ext";
import PinnedFrame from "../console/plots/PinnedFrame";
import { getThemeBackground } from "../navbar/theme/util";

const GamePlot = () => (
	<div class={`box ${getThemeBackground()}`}>
		<PinnedFrame
			isConsole={false}
			apiUrl={BENCHER_CLOUD_API_URL}
			user={defaultUser}
			project_slug={() => "game"}
			plot={
				{
					uuid: "2dcb48e3-1351-4aa8-9d98-a888eb0a62b9",
					project: "51fa020f-6843-47be-b374-17863cab5158",
					branches: ["3a27b3ce-225c-4076-af7c-75adbc34ef9a"],
					testbeds: ["bc05ed88-74c1-430d-b96a-5394fdd18bb0"],
					benchmarks: ["077449e5-5b45-4c00-bdfb-3a277413180d"],
					measures: ["52507e04-ffd9-4021-b141-7d4b9f1e9194"],
					lower_value: false,
					upper_value: false,
					lower_boundary: false,
					upper_boundary: true,
					x_axis: XAxis.DateTime,
					window: Date.now() / 1_000 - 1_697_414_400,
					created: "2024-11-24T19:26:45Z",
					modified: "2024-11-24T19:32:41Z",
				} as JsonPlot
			}
			logo={true}
		/>
	</div>
);

export default GamePlot;
