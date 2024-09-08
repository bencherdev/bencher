import { type Resource, Show } from "solid-js";
import { fmtDate } from "../../../../util/convert";
import { BACK_PARAM, encodePath, pathname } from "../../../../util/url";

export interface Props {
	redirect: (pathname: string, data: object) => string;
	data: Resource<object>;
}

const ReplacedButton = (props: Props) => {
	return (
		<Show when={props?.data()?.model?.replaced}>
			<div class="columns">
				<div class="column">
					<div class="notification is-warning">
						<div class="columns is-vcentered">
							<div class="column">
								<p>
									This threshold model was replaced on{" "}
									{fmtDate(props?.data()?.model?.replaced)}
								</p>
							</div>
							<div class="column is-narrow">
								<a
									class="button is-small"
									href={`${props.redirect?.(
										pathname(),
										props.params,
									)}?${BACK_PARAM}=${encodePath()}`}
								>
									<span class="fa-stack fa-2x" style="font-size: 1.0em;">
										<i class="fas fa-walking fa-stack-1x" />
										<i class="fas fa-ban fa-stack-2x" />
									</span>
									<span> View Current Threshold</span>
								</a>
							</div>
						</div>
					</div>
				</div>
			</div>
		</Show>
	);
};

export default ReplacedButton;
