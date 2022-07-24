import { Match, Switch } from "solid-js";
import { Link, Navigate } from "solid-app-router";

const ProjectsPanel = (props) => {
  props.handleTitle("Your Projects");

  return <p>TODO list all projects here</p>;
};

export default ProjectsPanel;
