import type { Accessor } from "solid-js";

export enum PaginationSize {
	SMALL = "is-small",
	REGULAR = "",
	MEDIUM = "is-medium",
	LARGE = "is-large",
}

const Pagination = (props: {
	size: PaginationSize;
	data_len: Accessor<undefined | number>;
	per_page: Accessor<number>;
	page: Accessor<number>;
	handlePage: (page: number) => void;
}) => {
	return (
		<nav
			class={`pagination is-centered is-rounded ${props.size}`}
			role="navigation"
			aria-label="pagination"
		>
			<button
				class="pagination-previous"
				title="Go to previous page"
				disabled={props.page() < 2}
				onClick={(e) => {
					e.preventDefault();
					props.handlePage(props.page() - 1);
				}}
			>
				Previous
			</button>
			<ul class="pagination-list">
				{props.page() > 2 && (
					<li>
						<button
							class="pagination-link"
							title="Go to page 1"
							onClick={(e) => {
								e.preventDefault();
								props.handlePage(1);
							}}
						>
							1
						</button>
					</li>
				)}
				{props.page() > 3 && (
					<li>
						<span class="pagination-ellipsis">&hellip;</span>
					</li>
				)}
				{props.page() > 1 && (
					<li>
						<button
							class="pagination-link"
							title={`Go to page ${props.page() - 1}`}
							onClick={(e) => {
								e.preventDefault();
								props.handlePage(props.page() - 1);
							}}
						>
							{props.page() - 1}
						</button>
					</li>
				)}
				<li>
					<button
						class="pagination-link is-current"
						title={`Page ${props.page()}`}
						aria-current="page"
					>
						{props.page() ? props.page() : 0}
					</button>
				</li>
				{props.data_len() === props.per_page() && (
					<li>
						<button
							class="pagination-link"
							title={`Go to page ${props.page() + 1}`}
							onClick={(e) => {
								e.preventDefault();
								props.handlePage(props.page() + 1);
							}}
						>
							{props.page() + 1}
						</button>
					</li>
				)}
			</ul>
			<button
				class="pagination-next"
				title="Go to next page"
				disabled={props.data_len() < props.per_page()}
				onClick={(e) => {
					e.preventDefault();
					props.handlePage(props.page() + 1);
				}}
			>
				Next page
			</button>
		</nav>
	);
};

export default Pagination;
