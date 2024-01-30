import { pathname, useNavigate } from "../../../util/url";

export interface Props {
	config: BillingHeaderConfig;
}

export interface BillingHeaderConfig {
	title: string;
	path: (pathname: string) => string;
}

const BillingHeader = (props: Props) => {
	const navigate = useNavigate();
	return (
		<nav class="level">
			<div class="level-left">
				<button
					class="button is-outlined"
					type="button"
					onClick={(e) => {
						e.preventDefault();
						navigate(props.config?.path(pathname()));
					}}
				>
					<span class="icon">
						<i class="fas fa-chevron-left" aria-hidden="true" />
					</span>
					<span>Back</span>
				</button>
			</div>
			<div class="level-left">
				<div class="level-item">
					<h3 class="title is-3">{props.config?.title}</h3>
				</div>
			</div>

			<div class="level-right" />
		</nav>
	);
};

export default BillingHeader;
