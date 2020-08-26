const templates = {
  // Template UUID
  t1: {
    // The Template ID
    id: "t1",
    // The Workflow ID that the Template is defined in
    workflow: "w1",
    // Whether the Template is hidden outside of its defining Workflow
    // The default value is true
    hidden: false,
    // The name of the Template
    name: "Balance Sheet",
    // A bascic description of the Flow
    description:
      "A Template for a Balance Sheet. It has three columns: Assets, Liabilities, and Stock Holder Equity.",
    // A list of UUIDs for the collaborators for the Template
    collaborators: ["u1"],
    // A list of UUIDs for the dependency Templates used in alphanumeric order
    // This will be useful for Complex Templates
    // A Complex Template has another Template embedded inside of it, as a column type
    // This is similar to Go struct embedding and is not OO inheritence.
    // And it is also the same way that untemplated Complex Tables will work.
    templates: [],
    // A map of dependency Templates, includes semver, timestamp, and checksum
    lock: {},
    // The Template Signature is just a named, reusable Table Signature really
    signature: {
      columns: {
        // The Columns that are to be set and are visibile outside of the Workflow for the Template
        // This is similar to how Go and Rust struct members work
        // Unlike traditional OO the level of encapsulation is at the package/module/Workflow level
        // It is not at the object/Template level
        visible: ["t1t1h1", "t1t1h2", "t1t1h3"],
        // The Columns that are only read and set by the Template outside of the defining Workflow
        // They are hidden outside of the Template when used outside of its defining Workflow
        // This leads to a higher level of encapsulation than just object based encapsulation
        hidden: ["t1t1h4"],
      },
      headers: {
        visible: {
          t1t1h1: { id: "t1t1h1", name: "Assets", type: "Number" },
          t1t1h2: { id: "t1t1h2", name: "Liabilities", type: "Number" },
          t1t1h3: { id: "t1t1h3", name: "Stock Holder Equity", type: "Number" },
        },
        hidden: {
          t1t1h4: { id: "t1t1h4", name: "Discrepency", type: "Number" },
        },
      },
    },
    // Template Subflows are like methods for a Template object
    // Like in OO these allow for Flows that have a close relationship with a Template to easily be grouped together
    // Template SubFlows will not be directly stored here
    // Rather the UUID for the Template Subflow will be provided here
    // and the Template Subflow itself will be stored along with the other Subflows of the defining Workflow
    // This is similar to how Decision Flows are handled
    subflows: {
      visible: [],
      hidden: [],
    },
  },
}

export default templates
