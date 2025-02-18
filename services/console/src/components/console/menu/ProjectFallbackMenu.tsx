import ProjectMenuInner from "./ProjectMenuInner";

const ProjectFallbackMenu = () => {
	return (
		<ProjectMenuInner
			project={() => undefined}
			active_alerts={() => undefined}
		/>
	);
};

export default ProjectFallbackMenu;
