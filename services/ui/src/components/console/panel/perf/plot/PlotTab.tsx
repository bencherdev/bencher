import { Link } from "solid-app-router";
import { For, Match, Show, Switch } from "solid-js";
import { PerfTab } from "../../../config/types";
import { toCapitalized } from "../../../config/util";

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
			<p class="panel-tabs">
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
			</p>
			<Switch
				fallback={
					<div class="box">
						<p>No {props.tab()} found</p>
					</div>
				}
			>
				<Match when={props.is_console && getTab().length === 0}>
					<div class="box">
						<AddButton project_slug={props.project_slug} tab={props.tab} />
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
				return "Create a Branch";
			case PerfTab.TESTBEDS:
				return "Create a Testbed";
			case PerfTab.BENCHMARKS:
				return "Track Your Benchmarks";
			default:
				return "Ello Poppet";
		}
	};

	return (
		<Link class="button is-primary is-fullwidth" href={getHref()}>
			{getText()}
		</Link>
	);
};

export default PlotTab;
