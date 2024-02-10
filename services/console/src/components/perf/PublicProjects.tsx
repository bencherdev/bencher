import {
	type Accessor,
	For,
	createEffect,
	createMemo,
	createResource,
	Show,
} from "solid-js";
import type { JsonProject } from "../../types/bencher";
import { authUser } from "../../util/auth";
import { httpGet } from "../../util/http";
import { useSearchParams } from "../../util/url";
import { DEBOUNCE_DELAY, validU32 } from "../../util/valid";
import Pagination, { PaginationSize } from "../site/Pagination";
import Field from "../field/Field";
import FieldKind from "../field/kind";
import { debounce } from "@solid-primitives/scheduled";

// const SORT_PARAM = "sort";
// const DIRECTION_PARAM = "direction";
const PER_PAGE_PARAM = "per_page";
const PAGE_PARAM = "page";
const SEARCH_PARAM = "search";

const DEFAULT_PER_PAGE = 8;
const DEFAULT_PAGE = 1;

export interface Props {
	apiUrl: string;
}

const PublicProjects = (props: Props) => {
	const [searchParams, setSearchParams] = useSearchParams();

	createEffect(() => {
		const initParams: Record<string, null | number> = {};
		if (!validU32(searchParams[PER_PAGE_PARAM])) {
			initParams[PER_PAGE_PARAM] = DEFAULT_PER_PAGE;
		}
		if (!validU32(searchParams[PAGE_PARAM])) {
			initParams[PAGE_PARAM] = DEFAULT_PAGE;
		}
		if (typeof searchParams[SEARCH_PARAM] !== "string") {
			initParams[SEARCH_PARAM] = null;
		}
		if (Object.keys(initParams).length !== 0) {
			setSearchParams(initParams, { replace: true });
		}
	});

	const per_page = createMemo(() => Number(searchParams[PER_PAGE_PARAM]));
	const page = createMemo(() => Number(searchParams[PAGE_PARAM]));
	const search = createMemo(() => searchParams[SEARCH_PARAM]);

	const pagination = createMemo(() => {
		return {
			per_page: per_page(),
			page: page(),
			public: true,
			search: search(),
		};
	});
	const fetcher = createMemo(() => {
		return {
			pagination: pagination(),
			token: authUser()?.token,
		};
	});
	const fetchProjects = async (fetcher: {
		pagination: {
			per_page: number;
			page: number;
			public: boolean;
			search: undefined | string;
		};
		token: undefined | string;
	}) => {
		const EMPTY_ARRAY: JsonProject[] = [];
		const searchParams = new URLSearchParams();
		for (const [key, value] of Object.entries(fetcher.pagination)) {
			if (value) {
				searchParams.set(key, value.toString());
			}
		}
		const path = `/v0/projects?${searchParams.toString()}`;
		return await httpGet(props.apiUrl, path, fetcher.token)
			.then((resp) => resp?.data)
			.catch((error) => {
				console.error(error);
				return EMPTY_ARRAY;
			});
	};
	const [projects] = createResource<JsonProject[]>(fetcher, fetchProjects);
	const projectsLength = createMemo(() => projects()?.length);

	const handlePage = (page: number) => {
		if (validU32(page)) {
			setSearchParams({ [PAGE_PARAM]: page }, { scroll: true });
		}
	};

	const handleSearch = debounce(
		(search: string) =>
			setSearchParams(
				{ [PAGE_PARAM]: DEFAULT_PAGE, [SEARCH_PARAM]: search },
				{ scroll: true },
			),
		DEBOUNCE_DELAY,
	);

	return (
		<section class="section">
			<div class="container">
				<div class="columns is-mobile">
					<div class="column">
						<div class="content">
							<h1 class="title is-1">Projects</h1>
							<hr />
							<Field
								kind={FieldKind.SEARCH}
								fieldKey="search"
								value={search() ?? ""}
								config={{
									placeholder: "Search Projects",
								}}
								handleField={(_key, search, _valid) =>
									handleSearch(search as string)
								}
							/>
							<br />
							<Show
								when={projectsLength() > 0}
								fallback={
									<div class="box">
										<p>🐰 No projects found</p>
									</div>
								}
							>
								<For each={projects()}>
									{(project) => (
										<a
											class="box"
											title={`View ${project.name}`}
											href={`/perf/${project.slug}`}
										>
											{project.name}
										</a>
									)}
								</For>
							</Show>
							{projectsLength() === 0 && page() !== 1 && (
								<div class="box">
									<BackButton page={page} handlePage={handlePage} />
								</div>
							)}
							<br />
						</div>
					</div>
				</div>
				<Pagination
					size={PaginationSize.REGULAR}
					data_len={projectsLength}
					per_page={per_page}
					page={page}
					handlePage={handlePage}
				/>
			</div>
		</section>
	);
};

const BackButton = (props: {
	page: Accessor<number>;
	handlePage: (page: number) => void;
}) => {
	return (
		<button
			class="button is-primary is-fullwidth"
			type="button"
			onClick={(e) => {
				e.preventDefault();
				props.handlePage(props.page() - 1);
			}}
		>
			That's all the projects. Go back.
		</button>
	);
};

export default PublicProjects;
