const workflows = {
  // Workflow UUID
  w1: {
    // The Workflow ID
    id: "w1",
    // The name of the Workflow
    name: "Double Sided Balance Sheet",
    // A bascic description of the Workflow
    description: "A Workflow for calculating a double sided balance sheet",
    // A list of UUIDs for the collaborators for the Workflow
    collaborators: ["u1"],
    // A list of UUIDs for the dependency Workflows used in alphanumeric order
    workflows: [],
    // A map of dependency Workflows, includes semver, timestamp, and checksum
    lock: {},
    // The Flows, Templates, and Contracts declared in the Workflow
    flows: {},
    templates: {},
    contracts: {},
  },
}

export default workflows
