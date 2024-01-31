import type { Accessor } from "solid-js";

export interface Props {
	start_date: Accessor<undefined | string>;
	end_date: Accessor<undefined | string>;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
}

const DateRange = (props: Props) => {
	return (
		<div class="columns is-centered">
			<div class="column is-narrow">
				<div class="level is-mobile">
					<div class="level-item">
						<div class="columns">
							<div class="column">
								<p>Search Start Date</p>
								<input
									title="Search Start Date"
									type="date"
									value={props.start_date() ?? ""}
									onInput={(e) => props.handleStartTime(e.currentTarget?.value)}
								/>
							</div>
						</div>
					</div>
					<div class="level-item">
						<div class="columns">
							<div class="column">
								<p>Search End Date</p>
								<input
									title="Search End Date"
									type="date"
									value={props.end_date() ?? ""}
									onInput={(e) => props.handleEndTime(e.currentTarget?.value)}
								/>
							</div>
						</div>
					</div>
				</div>
			</div>
		</div>
	);
};

export default DateRange;
