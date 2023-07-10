import axios from "axios";
import { Link, useSearchParams } from "solid-app-router";
import { createEffect, createMemo, createResource, For } from "solid-js";
import {
	BENCHER_API_URL,
	get_options,
	pageTitle,
	validate_jwt,
	validate_u32,
} from "../site/util";
import ProjectsFooter from "./ProjectsFooter";

// const SORT_PARAM = "sort";
// const DIRECTION_PARAM = "direction";
const PER_PAGE_PARAM = "per_page";
const PAGE_PARAM = "page";

const DEFAULT_PER_PAGE = 8;
const DEFAULT_PAGE = 1;

const ProjectsPage = (props) => {
	const [searchParams, setSearchParams] = useSearchParams();

	if (!validate_u32(searchParams[PER_PAGE_PARAM])) {
		setSearchParams({ [PER_PAGE_PARAM]: DEFAULT_PER_PAGE });
	}
	if (!validate_u32(searchParams[PAGE_PARAM])) {
		setSearchParams({ [PAGE_PARAM]: DEFAULT_PAGE });
	}

	const per_page = createMemo(() => Number(searchParams[PER_PAGE_PARAM]));
	const page = createMemo(() => Number(searchParams[PAGE_PARAM]));

	const pagination_query = createMemo(() => {
		return {
			per_page: per_page(),
			page: page(),
			public: true,
		};
	});
	const fetcher = createMemo(() => {
		return {
			pagination_query: pagination_query(),
			token: props.user?.token,
		};
	});
	const fetchProjects = async (fetcher) => {
		const EMPTY_ARRAY = [];
		if (fetcher.token && !validate_jwt(fetcher.token)) {
			return EMPTY_ARRAY;
		}
		if (!validate_u32(fetcher.pagination_query.per_page.toString())) {
			return EMPTY_ARRAY;
		}
		if (!validate_u32(fetcher.pagination_query.page.toString())) {
			return EMPTY_ARRAY;
		}
		const search_params = new URLSearchParams();
		for (const [key, value] of Object.entries(fetcher.pagination_query)) {
			if (value) {
				search_params.set(key, value);
			}
		}
		const url = `${BENCHER_API_URL()}/v0/projects?${search_params.toString()}`;
		return await axios(get_options(url, fetcher.token))
			.then((resp) => resp?.data)
			.catch((error) => {
				console.error(error);
				return EMPTY_ARRAY;
			});
	};
	const [projects] = createResource(fetcher, fetchProjects);

	createEffect(() => {
		pageTitle("Projects");
	});
	createEffect(() => {
		if (!validate_u32(searchParams[PER_PAGE_PARAM])) {
			setSearchParams({ [PER_PAGE_PARAM]: DEFAULT_PER_PAGE });
		}
	});
	createEffect(() => {
		if (!validate_u32(searchParams[PAGE_PARAM])) {
			setSearchParams({ [PAGE_PARAM]: DEFAULT_PAGE });
		}
	});

	const handlePage = (page: number) => {
		if (validate_u32(page.toString())) {
			setSearchParams({ [PAGE_PARAM]: page });
		}
	};

	return (
		<section class="section">
			<div class="container">
				<div class="columns is-mobile">
					<div class="column">
						<div class="content">
							<h2 class="title">Projects</h2>
							<hr />
							<br />
							<For each={projects()}>
								{(project) => (
									<Link class="box" href={`/perf/${project.slug}`}>
										{project.name}
									</Link>
								)}
							</For>
							{projects()?.length === 0 && page() !== 1 && (
								<div class="box">
									<BackButton page={page} handlePage={handlePage} />
								</div>
							)}
							<br />
						</div>
					</div>
				</div>
				<ProjectsFooter
					page={page}
					per_page={per_page}
					handlePage={handlePage}
					table_data_len={projects()?.length}
				/>
			</div>
		</section>
	);
};

const BackButton = (props) => {
	return (
		<button
			class="button is-primary is-fullwidth"
			onClick={(e) => {
				e.preventDefault();
				props.handlePage(props.page() - 1);
			}}
		>
			That's all the projects. Go back.
		</button>
	);
};

export default ProjectsPage;
