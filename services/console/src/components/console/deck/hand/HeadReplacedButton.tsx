import { type Resource, Show } from "solid-js";
import { fmtDate } from "../../../../util/convert";
import { BACK_PARAM, encodePath, pathname } from "../../../../util/url";
import { BRANCH_ICON } from "../../../../config/project/branches";

export interface Props {
	data: Resource<object>;
}

const HeadReplacedButton = (props: Props) => {
	return (
		<Show when={props?.data()?.head?.replaced}>
			<div class="columns">
				<div class="column">
					<div class="notification is-warning">
						<div class="columns is-vcentered">
							<div class="column">
								<p>
									This branch head reference was replaced on{" "}
									{fmtDate(props?.data()?.head?.replaced)}
								</p>
							</div>
							<div class="column is-narrow">
								<a
									class="button is-small"
									href={`${pathname()}?${BACK_PARAM}=${encodePath()}`}
								>
									<span class="icon">
										<i class={BRANCH_ICON} />
									</span>
									<span>View Current Branch</span>
								</a>
							</div>
						</div>
					</div>
				</div>
			</div>
		</Show>
	);
};

export default HeadReplacedButton;
