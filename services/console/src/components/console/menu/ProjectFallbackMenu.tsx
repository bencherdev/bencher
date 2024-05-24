import ProjectMenuInner from "./ProjectMenuInner";

const ProjectFallbackMenu = () => {
	return <ProjectMenuInner project={() => undefined} active_alerts={() => 0} />;
};

export default ProjectFallbackMenu;
