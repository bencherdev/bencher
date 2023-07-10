import { Link } from "solid-app-router";
import { For, Match, Show, Switch } from "solid-js";
import { PerfTab } from "../../../config/types";
import { toCapitalized } from "../../../config/util";
import TabFooter from "./TabFooter";
import { DEFAULT_PAGE } from "../PerfPanel";

const perf_tabs = [PerfTab.BRANCHES, PerfTab.TESTBEDS, PerfTab.BENCHMARKS];

const PlotTab = (props) => {
	const getTab = () => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				return props.branches_tab;
			case PerfTab.TESTBEDS:
				return props.testbeds_tab;
			case PerfTab.BENCHMARKS:
				return props.benchmarks_tab;
			default:
				return [];
		}
	};

	const getPage = () => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				return props.branches_page;
			case PerfTab.TESTBEDS:
				return props.testbeds_page;
			case PerfTab.BENCHMARKS:
				return props.benchmarks_page;
			default:
				return 1;
		}
	};

	const getHandlePage = () => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				return props.handleBranchesPage;
			case PerfTab.TESTBEDS:
				return props.handleTestbedsPage;
			case PerfTab.BENCHMARKS:
				return props.handleBenchmarksPage;
			default:
				return (page: number) =>
					console.error("No handle for tab", props.tab(), page);
		}
	};

	const handleChecked = (index: number, uuid: string) => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				return props.handleBranchChecked(index, uuid);
			case PerfTab.TESTBEDS:
				return props.handleTestbedChecked(index, uuid);
			case PerfTab.BENCHMARKS:
				return props.handleBenchmarkChecked(index, uuid);
			default:
				return [];
		}
	};

	return (
		<>
			<div class="panel-tabs">
				<For each={perf_tabs}>
					{(tab) => (
						<a
							class={props.tab() === tab ? "is-active" : ""}
							onClick={() => props.handleTab(tab)}
						>
							{toCapitalized(tab)}
						</a>
					)}
				</For>
			</div>
			<Switch
				fallback={
					<div class="box">
						<p>No {props.tab()} found</p>
					</div>
				}
			>
				<Match
					when={
						props.is_console &&
						getTab().length === 0 &&
						getPage() === DEFAULT_PAGE
					}
				>
					<div class="box">
						<AddButton project_slug={props.project_slug} tab={props.tab} />
					</div>
				</Match>
				<Match
					when={
						props.is_console &&
						getTab().length === 0 &&
						getPage() !== DEFAULT_PAGE
					}
				>
					<div class="box">
						<BackButton
							tab={props.tab}
							page={getPage()}
							handlePage={getHandlePage()}
						/>
					</div>
				</Match>
				<Match when={getTab().length > 0}>
					<For each={getTab()}>
						{(item, index) => (
							// TODO maybe move over to a store and see if this works?
							// https://www.solidjs.com/tutorial/stores_createstore
							<a
								class="panel-block"
								onClick={(e) => handleChecked(index(), item.uuid)}
							>
								<div class="columns is-vcentered is-mobile">
									<div class="column is-narrow">
										<input type="checkbox" checked={item.checked} />
									</div>
									<div class="column">
										<small style="overflow-wrap:anywhere;">{item.name}</small>
									</div>
								</div>
							</a>
						)}
					</For>
				</Match>
			</Switch>
			<div class="panel-block">
				<TabFooter
					per_page={props.per_page}
					page={getPage()}
					handlePage={getHandlePage()}
					table_data_len={getTab().length}
				/>
			</div>
		</>
	);
};

const AddButton = (props) => {
	const getHref = () => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
			case PerfTab.TESTBEDS:
				return `/console/projects/${props.project_slug}/${props.tab()}/add`;
			case PerfTab.BENCHMARKS:
				return "/docs/how-to/track-benchmarks";
			default:
				return "#";
		}
	};

	const getText = () => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				return "Add a Branch";
			case PerfTab.TESTBEDS:
				return "Add a Testbed";
			case PerfTab.BENCHMARKS:
				return "Track Your Benchmarks";
			default:
				return "Unknown Tab";
		}
	};

	return (
		<Link class="button is-primary is-fullwidth" href={getHref()}>
			{getText()}
		</Link>
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
			That's all the {props.tab()}. Go back.
		</button>
	);
};

export default PlotTab;
