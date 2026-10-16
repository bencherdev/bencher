import Button from "@bencherdev/ui/Button";
import { Show } from "solid-js";

/** A spacer row only holds the height of the rows not drawn. */
export const Spacer = (props: { height: number; columns: number }) => (
	<Show when={props.height > 0}>
		{/* biome-ignore lint/a11y/noAriaHiddenOnFocusable: a spacer row only holds the height of the rows not drawn */}
		<tr
			class="reports-spacer"
			aria-hidden="true"
			style={{ height: `${props.height}px` }}
		>
			<td colSpan={props.columns} />
		</tr>
	</Show>
);

/** The batch after the rows drawn: a placeholder while it loads, a Retry once it failed. */
export const MoreRows = (props: {
	columns: number;
	loading: boolean;
	onRetry?: (() => void) | undefined;
	/** What did not load, such as "These runs". */
	failed: string;
}) => (
	<>
		<Show when={props.loading}>
			{/* biome-ignore lint/a11y/noAriaHiddenOnFocusable: a placeholder row has nothing to focus or read */}
			<tr class="reports-spacer reports-more" aria-hidden="true">
				<td colSpan={props.columns}>
					<span class="ui-skeleton" data-size="text" />
				</td>
			</tr>
		</Show>
		<Show when={!props.loading && props.onRetry}>
			{(retry) => (
				<tr class="reports-more">
					<td colSpan={props.columns}>
						<div class="reports-failed" role="alert">
							<span>{props.failed} did not load</span>
							<Button size="sm" onClick={() => retry()()}>
								Retry
							</Button>
						</div>
					</td>
				</tr>
			)}
		</Show>
	</>
);
