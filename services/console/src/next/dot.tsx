/** The dot between the parts of a `.ui-parts` label, which keeps its width; the spaces read the parts as words. */
const Dot = () => (
	<>
		{" "}
		<span data-fixed aria-hidden="true">
			·
		</span>{" "}
	</>
);

export default Dot;
