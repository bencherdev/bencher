import { For, Show } from "solid-js";
import Tag from "../tag";

/** A parameters filter's sets as tags, "or" between them. */
const Sets = (props: { sets: string[][]; reveal?: boolean }) => (
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
								<Tag tag={tag} reveal={props.reveal === true} />
							</>
						)}
					</For>
				</>
			)}
		</For>
	</span>
);

export default Sets;
