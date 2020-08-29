import dashboardTabs from "./dashboardTabs"
import workflowsTabs from "./workflowsTabs"
import flowsTabs from "./flowsTabs"
import templatesTabs from "./templatesTabs"
import contractsTabs from "./contractsTabs"

const studioSection = (path: string) => {
  return {
    title: "Studio",
    tabs: [
      dashboardTabs.dashboard(path),
      workflowsTabs.workflows(path),
      workflowsTabs.new(path),
      flowsTabs.flows(path),
      flowsTabs.new(path),
      templatesTabs.templates(path),
      templatesTabs.new(path),
      contractsTabs.contracts(path),
      contractsTabs.new(path),
    ],
  }
}

export default studioSection
