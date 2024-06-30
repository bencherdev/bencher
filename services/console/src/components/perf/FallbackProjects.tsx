import { For } from "solid-js";
import Pagination, { PaginationSize } from "../site/Pagination";

const DEFAULT_PER_PAGE = 8;

const FallbackProjects = () => {
	return (
		<section class="section">
			<div class="container">
				<div class="columns">
					<div class="column">
						<div class="content">
							<h1 class="title is-1">Projects</h1>
							<hr />
							<div class="control has-icons-left has-icons-right">
								<span class="icon is-small is-left">
									<i class="fas fa-search" />
								</span>
								<input
									class="input"
									type="search"
									placeholder="Search Projects"
									value=""
								/>
							</div>
							<br />
							<For each={Array(DEFAULT_PER_PAGE)}>
								{() => (
									<div class="box">
										<p>&nbsp;</p>
									</div>
								)}
							</For>
							<br />
						</div>
					</div>
				</div>
				<Pagination
					size={PaginationSize.REGULAR}
					data_len={() => 0}
					per_page={() => DEFAULT_PER_PAGE}
					page={() => 1}
					total_count={() => 0}
					handlePage={(_page) => {}}
				/>
			</div>
		</section>
	);
};

export default FallbackProjects;
