import Skeleton from "@bencherdev/ui/Skeleton";

/** A page's place before its code or data arrives. */
const ContentSkeleton = () => (
	<main class="page" aria-busy="true">
		<div class="pagehead">
			<Skeleton size="text" class="title-skel" />
		</div>
		<Skeleton size="card" />
	</main>
);

export default ContentSkeleton;
