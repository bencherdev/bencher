import { For, Show } from "solid-js";

/** A parameters filter's sets as tags, "or" between them. */
const Sets = (props: { sets: string[][] }) => (
	<span class="th-sets">
		<For each={props.sets}>
			{(tags, set) => (
				<>
					<Show when={set() > 0}>
						<span class="th-or"> or </span>
					</Show>
					<For each={tags}>
						{(tag, position) => (
							<>
								<Show when={position() > 0}> </Show>
								<span class="lr-ptag">{tag}</span>
							</>
						)}
					</For>
				</>
			)}
		</For>
	</span>
);

export default Sets;
