const flows = {
  // Flow UUID
  a: {
    // The Flow ID
    id: "a",
    // The ID for the main Flow in the Subflows
    main: "a1",
    // The name of the Flow
    name: "Hello, Math!",
    // A bascic description of the Flow
    description:
      "A kitchen sink example of all TableFlow Elements doing very basic operations. Eventually this will turn into the foundation for the Tour of TableFlow",
    // A list of UUIDs for the collaborators for the Flow
    collaborators: ["u1"],
    // A list of UUIDs for the dependency Flows used in alphanumeric order
    flows: ["b"],
    // A map of dependency Flows, includes semver, timestamp, and checksum
    lock: {
      b: {
        id: "b",
        version: "v0.0.1",
        timestamp: "20200810221718",
        sum: "G0oB1YgO0k",
      },
    },
    // A map of all of the Subflows within a Flow
    subflows: {
      // A map of all Subflows
      a1: {
        // The Subflow ID
        id: "a1",
        // The Subflow name
        // the first Subflow by convention is called `Main`
        // but its name can be changed like all other Subflows
        name: "Main",
        // The Subflows Parent Subflow ID
        // This will be a blank string for Main Subflows
        parent: "",
        // The Input Element ID for the Subflow
        // This can be cross-checked with the order below
        // For the Main Subflow, it should be the first element
        // but for all other Subflows it should be the second
        input: "e0",
        // The Output Element for the Subflow
        // This can be cross-checked with the order below
        // It should always be last
        output: "e1",
        // The order of elements in the Subflow
        order: ["e0", "e3", "e5", "e8", "e11", "e13", "e1"],
        // Each Element is its own object
        // TODO break out Elements and variables
        elements: {
          // Need a for Flow inputs
          e0: {
            // There will always be an Input Element
            // even if it is unused
            // The id of the element
            id: "e0",
            // The type of the Element
            // Each type will have a different value
            // TODO make these in Typescript
            type: "input",
            // The value of the Element
            // each Element type may have different keys here
            value: {
              inputs: [
                "v2",
                "v7",
                // "v12", TODO add back once Table format changed
              ],
            },
          },
          e1: {
            // There will always be a Output Element
            // even if it is unused
            id: "e1",
            type: "output",
            value: {
              outputs: ["v4", "v10"],
            },
          },
          e3: {
            id: "e3",
            type: "decision",
            value: {
              name: "Square Formula",
              inputs: ["v2"],
              outputs: ["v4"],
              columns: {
                inputs: ["e3h1"],
                outputs: ["e3h2"],
              },
              headers: {
                inputs: {
                  e3h1: {
                    id: "e3h1",
                    table: "v2",
                    column: "v2h1",
                    // A comma separated list of conditions
                    // These are evaluated with AND logic
                    // The conditions variable depend on the column type
                    // Eventually create a UI wrapper for the complex ones
                    conditions: "=",
                  },
                },
                outputs: {
                  e3h2: {
                    id: "e3h2",
                    table: "v4",
                    column: "v4h1",
                  },
                },
              },
              // The inputs are literal values
              // They evaluate to true/false based off of all the column's conditions
              // The conditions for all columns are evaluated using AND logic
              // `-` or `*` are special "match all" characters
              // Strings need to be in single or double quotes `''` or `""`
              // The outputs are expressions
              // They may use simple arithmetic operators: +, -, *, /, %, and ^
              // Sum, Minimum, Maximum, Count, and Average functions
              // should be available as well
              // Table and column names may be referenced with snake_case
              // based off of their respective names using dot notation
              // Each row is evaluated with OR logic
              // The first case to match is the one evaluated
              // A `?` is used to trigger the creation of a Decision Subflow for a case's output
              // Decision Subflows are stored as a URI for the Subflow (subflow://[subflowId])
              // This Subflow ID value will have a nice GUI wrapper
              // If a value doesn't match any of the cases,
              // then the zero value for the type will be placed in the output table
              rows: [
                {
                  inputs: { e3h1: "0" },
                  outputs: { e3h2: "subflow://a3" },
                },
                {
                  inputs: { e3h1: "-" },
                  outputs: { e3h2: "input_table.value^2" },
                },
              ],
              // Sum, Minimum, Maximum, Count, and Average functions
              // These functions will be available when the output type is a Locked Row only.
              // This helps to solve the cross-row/columnar problem.
            },
          },
          e5: {
            id: "e5",
            type: "function",
            value: {
              id: "b",
              inputs: ["v4", ""],
              outputs: ["v6"],
            },
          },
          e8: {
            id: "e8",
            type: "subflow",
            value: {
              id: "a2",
              inputs: ["v7"],
              outputs: ["v9"],
            },
          },
          e11: {
            id: "e11",
            type: "row",
            value: {
              id: "v10",
            },
          },
          e13: {
            id: "e13",
            type: "chart",
            value: {
              id: "v12",
              config: {
                // TODO add the D3 config options here
              },
            },
          },
        },
        declarations: ["v2", "v7", "v12", "v4", "v6", "v9", "v10"],
        variables: {
          v2: {
            id: "v2",
            type: "table",
            value: {
              name: "Input Table",
              columns: ["v2h1"],
              headers: {
                v2h1: { id: "v2h1", name: "Value", type: "Number" },
              },
              rows: [{ v2h1: 5 }],
            },
          },
          v4: {
            id: "v4",
            type: "table",
            value: {
              name: "Output Table",
              columns: ["v4h1"],
              headers: {
                v4h1: { id: "v4h1", name: "Squared Value", type: "Number" },
              },
              rows: [{ v4h1: 25 }],
            },
          },
          v6: {
            id: "v6",
            type: "table",
            value: {
              name: "Function Output Table",
              columns: ["v6h1"],
              headers: {
                v6h1: {
                  id: "v6h1",
                  name: "Function Sum Value",
                  type: "Number",
                },
              },
              rows: [{ v6h1: 0 }],
            },
          },
          v7: {
            id: "v7",
            type: "table",
            value: {
              name: "The Question",
              columns: ["v7h1"],
              headers: {
                v7h1: { id: "v7h1", name: "Life", type: "String" },
              },
              rows: [{ v7h1: "What is the meaning of life?" }],
            },
          },
          v9: {
            id: "v9",
            type: "table",
            value: {
              name: "The Answer",
              columns: ["v9h1"],
              headers: {
                v9h1: { id: "v9h1", name: "Answer", type: "Number" },
              },
              rows: [{ v9h1: 42 }],
            },
          },
          v10: {
            id: "v10",
            type: "row",
            value: {
              name: "Locked Row",
              columns: ["v10h1"],
              headers: {
                v10h1: { id: "v10h1", name: "Pi", type: "Number" },
              },
              rows: [{ v10h1: 3.14159 }],
            },
          },
          v12: {
            id: "v12",
            type: "table",
            value: {
              name: "The Table to be Charted",
              columns: ["v12h1"],
              headers: {
                v12h1: { id: "v12h1", name: "Genre", type: "String" },
                v12h2: { id: "v12h2", name: "Revenue", type: "Number" },
              },
              rows: [
                { genre: "Alt", revenue: 5 },
                { genre: "Classical", revenue: 3 },
                { genre: "Disco", revenue: 7 },
                { genre: "R&B", revenue: 9 },
                { genre: "Rock", revenue: 1 },
                // TODO use real Table once Chart has been updated
                // { v12h1: "Alt", v12h2: 5 },
                // { v12h1: "Classical", v12h2: 3 },
                // { v12h1: "Disco", v12h2: 7 },
                // { v12h1: "R&B", v12h2: 9 },
                // { v12h1: "Rock", v12h2: 1 },
              ],
            },
          },
        },
      },
      a2: {
        id: "a2",
        name: "The Subflow to Answer Everything",
        parent: "a1",
        input: "a2e0",
        output: "a2e1",
        order: ["a2e2", "a2e0", "a2e1"],
        elements: {
          a2e0: {
            id: "a2e0",
            type: "input",
            value: {
              inputs: ["a2v3"],
            },
          },
          a2e1: {
            id: "a2e1",
            type: "output",
            value: {
              outputs: ["a2v4"],
            },
          },
          a2e2: {
            id: "a2e2",
            type: "parent",
            value: {
              id: "a1",
            },
          },
        },
        declarations: ["a2v3", "a2v4"],
        variables: {
          a2v3: {
            id: "a2v3",
            type: "table",
            value: {
              name: "The Question",
              columns: ["a2v3h1"],
              headers: {
                a2v3h1: { id: "a2v3h1", name: "Life", type: "String" },
              },
              rows: [{ a2v3h1: "What is the meaning of life?" }],
            },
          },
          a2v4: {
            id: "a2v4",
            type: "table",
            value: {
              name: "The Answer",
              columns: ["a2v4h1"],
              headers: {
                a2v4h1: { id: "a2v4h1", name: "Answer", type: "Number" },
              },
              rows: [{ a2v4h1: 42 }],
            },
          },
        },
      },
      a3: {
        id: "a3",
        name: "A Needless Decision Subflow",
        parent: "a1",
        input: "a3e0",
        output: "a3e1",
        order: ["a3e2", "a3e0", "a3e1"],
        elements: {
          a3e0: {
            id: "a3e0",
            type: "input",
            value: {
              inputs: ["a3v3"],
            },
          },
          a3e1: {
            id: "a3e1",
            type: "output",
            value: {
              outputs: ["a3v4"],
            },
          },
          a3e2: {
            id: "a3e2",
            type: "parent",
            value: {
              id: "a1",
            },
          },
        },
        declarations: ["a3v3", "a3v4"],
        variables: {
          a3v3: {
            id: "a3v3",
            type: "table",
            value: {
              // In a Decision Subflow the inputs to the Subflow
              // are exactly the same as the inputs to the parent Decision Table
              name: "Input Table",
              columns: ["a3v3h1"],
              headers: {
                a3v3h1: { id: "a3v3h1", name: "Value", type: "Number" },
              },
              rows: [{ a3v3h1: 0 }],
            },
          },
          a3v4: {
            id: "a3v4",
            type: "table",
            value: {
              // In a Decision Subflow the outputs from the Subflow
              // are exactly the same as the outputs from the parent Decision Table
              // ie Continue logic ->
              // or
              // they are exactly the same as the parent Subflow (the one with the Decision Table)
              // This allows a Decision Subflow to return from that parent Subflow
              // ie Return logic <-
              name: "Output Table",
              columns: ["a3v4h1"],
              headers: {
                a3v4h1: { id: "a3v4h1", name: "Squared Value", type: "Number" },
              },
              rows: [{ a3v4h1: 0 }],
            },
          },
        },
      },
    },
  },
  b: {
    id: "b",
    main: "b1",
    name: "Sum",
    description:
      "A really basic wrapper over addition to test out how Functions should work.",
    collaborators: ["u2"],
    flows: [],
    lock: {},
    subflows: {
      b1: {
        id: "b1",
        name: "Main",
        parent: "",
        input: "b1e0",
        output: "b1e1",
        order: ["b1e0", "b1e4", "b1e1"],
        elements: {
          b1e0: {
            id: "b1e0",
            type: "input",
            value: {
              inputs: ["b1v2", "b1v3"],
            },
          },
          b1e1: {
            id: "b1e1",
            type: "output",
            value: {
              outputs: ["b1v5"],
            },
          },
          b1e4: {
            id: "b1e4",
            type: "decision",
            value: {
              name: "Sum Formula",
              inputs: ["b1v2", "b1v3"],
              outputs: ["b1v5"],
              columns: {
                inputs: ["b1e4h1", "b1e4h2"],
                outputs: ["b1e4h3"],
              },
              headers: {
                inputs: {
                  b1e4h1: {
                    id: "b1e4h1",
                    table: "b1v2",
                    column: "b1v2h1",
                    conditions: "=",
                  },
                  b1e4h2: {
                    id: "b1e4h2",
                    table: "b1v3",
                    column: "b1v3h1",
                    conditions: "=",
                  },
                },
                outputs: {
                  b1e4h3: {
                    id: "b1e4h3",
                    table: "b1v5",
                    column: "b1v5h1",
                  },
                },
              },
              rows: [
                {
                  inputs: { b1e4h1: "*", b1e4h2: "*" },
                  outputs: { b1e4h3: "left_side.left + right_side.right" },
                },
              ],
            },
          },
        },
        declarations: ["b1v2", "b1v3", "b1v5"],
        variables: {
          b1v2: {
            id: "b1v2",
            type: "table",
            value: {
              name: "Left Side",
              columns: ["b1v2h1"],
              headers: {
                b1v2h1: { id: "b1v2h1", name: "Left", type: "Number" },
              },
              rows: [{ b1v2h1: 0 }],
            },
          },
          b1v3: {
            id: "b1v3",
            type: "table",
            value: {
              name: "Right Side",
              columns: ["b1v3h1"],
              headers: {
                b1v3h1: { id: "b1v3h1", name: "Right", type: "Number" },
              },
              rows: [{ b1v3h1: 0 }],
            },
          },
          b1v5: {
            id: "b1v5",
            type: "table",
            value: {
              name: "Sum Table",
              columns: ["b1v5h1"],
              headers: {
                b1v5h1: { id: "b1v5h1", name: "Sum", type: "Number" },
              },
              rows: [{ b1v5h1: 0 }],
            },
          },
        },
      },
    },
  },
}

export default flows
