import flowsTabs from "./flowsTabs"

const studioSection = (path: string) => {
  return {
    title: "Studio",
    tabs: [flowsTabs.dashboard(path, "Flows"), flowsTabs.new(path, "Flows")],
  }
}

export default studioSection
