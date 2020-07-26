import React from "react"

import Table from "./statictable"

const flows = {
  // Flow UUID
  a: {
    // The UUID for the main Flow in the Subflows
    main: "a1",
    // A map of all of the Subflows within a Flow
    subflows: {
      // A particular Subflow, may be the Main Flow
      a1: [
        // It is a list of lists
        // Think rows in a file for each Flow
        [
          // Each Element is its own object
          {
            // The type of the Element:
            // Table
            // Function
            // Decision Table
            // Subflow Reference
            type: "table",
            // Each type will have a different value
            // TODO make these in Typescript
            value: {
              name: "Input",
              var: "input",
              columns: [{ name: "Input", var: "input", type: "int" }],
              rows: [[5]],
            },
          },
        ],
      ],
    },
  },
  b: null,
  c: null,
}

const Modeler = (props: { path: string; id: string }) => {
  let date = Date()
  return (
    <div>
      <p>
        Modeler {props.path} {props.id} {date}
      </p>
      <Table />
    </div>
  )
}

export default Modeler
