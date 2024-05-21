import { decodePath, pathname } from "../../../util/url";

export interface Props {
	config: PosterHeaderConfig;
}

export interface PosterHeaderConfig {
	title: string;
	path: (pathname: string) => string;
	path_to: string;
}

const PosterHeader = (props: Props) => {
	return (
		<nav class="level">
			<div class="level-left">
				<a
					class="button"
					title={`Back to ${props.config?.path_to}`}
					href={decodePath(props.config?.path(pathname()))}
				>
					<span class="icon">
						<i class="fas fa-chevron-left" />
					</span>
					<span>Back</span>
				</a>
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

export default PosterHeader;
