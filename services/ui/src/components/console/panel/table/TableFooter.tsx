const TableFooter = (props) => {
	return (
		<section class="section">
			<div class="container">
				<nav
					class="pagination is-centered is-rounded"
					role="navigation"
					aria-label="pagination"
				>
					<button
						class="pagination-previous"
						aria-label="Go to previous page"
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
									aria-label="Go to page 1"
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
									aria-label={`Go to page ${props.page() - 1}`}
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
								aria-label={`Page ${props.page()}`}
								aria-current="page"
							>
								{props.page()}
							</button>
						</li>
						{props.table_data_len == props.per_page() && (
							<li>
								<button
									class="pagination-link"
									aria-label={`Go to page ${props.page + 1}`}
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
						aria-label="Go to next page"
						disabled={props.table_data_len < props.per_page()}
						onClick={(e) => {
							e.preventDefault();
							props.handlePage(props.page() + 1);
						}}
					>
						Next page
					</button>
				</nav>
			</div>
		</section>
	);
};

export default TableFooter;
