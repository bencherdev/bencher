import React, { useState, useEffect } from "react"
import { navigate } from "gatsby"
import { cloneDeep } from "lodash/lang"
import { Columns } from "react-bulma-components"

import Page from "./page"

import InterpreterWorker from "../../../interpreter/interpreter"
import SEO from "../../utils/seo"

const flows = {
  // Flow UUID
  a: {
    // The Flow ID
    id: "a",
    // The ID for the main Flow in the Subflows
    main: "a1",
    // The name of the Flow
    name: "Hello, Math!",
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
        order: ["e0", "e3", "e5", "e8", "e11", "e1"],
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
              inputs: ["v2", "v7"],
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
              // If a value doesn't match any of the cases,
              // then the zero value for the type will be placed in the output table
              rows: [{ inputs: ["-"], outputs: ["input_table.value^2"] }],
              // Sum, Minimum, Maximum, Count, and Average functions
              // should also be available for COLUMNS. How though?
            },
          },
          e5: {
            id: "e5",
            type: "function",
            value: {
              // TODO make Function element actually link to another Flow
              id: "",
              name: "Sum",
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
        },
        declarations: ["v2", "v7", "v4", "v6", "v9", "v10"],
        variables: {
          v2: {
            id: "v2",
            type: "table",
            value: {
              name: "Input Table",
              columns: ["v2h1", "v2h1", "v2h1", "v2h1", "v2h1", "v2h1"],
              headers: {
                v2h1: { id: "v2h1", name: "Value", type: "Number" },
              },
              rows: [
                [5, 4, 3, 2, 1, 0],
                [5, 4, 3, 2, 1, 0],
                [5, 4, 3, 2, 1, 0],
                [5, 4, 3, 2, 1, 0],
              ],
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
              rows: [[25]],
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
              rows: [[0]],
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
              rows: [["What is the meaning of life?"]],
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
              rows: [[42]],
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
              rows: [[3.14159]],
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
              rows: [["What is the meaning of life?"]],
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
              rows: [[42]],
            },
          },
        },
      },
    },
  },
  b: null,
  c: null,
}

const Notebook = () => {
  const [flow, setFlow] = useState({
    id: "",
    main: "",
    name: "",
    subflows: {},
  })
  const [subflowId, setSubflowId] = useState("")
  const [redirect, setRedirect] = useState(false)
  const [interpreter, setInterpreter] = useState()

  const date = Date()

  function handleFlow(id: string) {
    let currentFlow = flows?.[id]
    if (!currentFlow) {
      setRedirect(true)
    }
    let newFlow = { id: id, ...currentFlow }
    setFlow(newFlow)
    setSubflowId(flows?.[id]?.main)
    handleInterpreter(newFlow)
  }

  function handleSubflow(id: string) {
    setSubflowId(id)
  }

  function handleUpdate(
    id: string,
    value: any,
    updateSubflowId: string,
    type: "elements" | "variables"
  ) {
    if (flow?.subflows?.[updateSubflowId]?.[type]?.[id]?.value) {
      let newFlow = cloneDeep(flow)
      newFlow.subflows[updateSubflowId][type][id].value = value
      setFlow(newFlow)
    }
  }

  function handleElement(
    id: string,
    value: any,
    elementSubflowId: string = subflowId
  ) {
    handleUpdate(id, value, elementSubflowId, "elements")
  }

  function handleVariable(
    id: string,
    value: any,
    variableSubflowId: string = subflowId
  ) {
    handleUpdate(id, value, variableSubflowId, "variables")
  }

  function handleInterpreter(config: any) {
    console.log("New interpreter")
    InterpreterWorker.init(config)
      .then((interp: any, err: any) => {
        console.log("interpreter inited in modeler")
        if (err) {
          console.error(err)
          return
        }
        console.log(interp)
        setInterpreter(interp)
        console.log("Interpreter set")
      })
      .catch((err: any) => console.error(err))
  }

  function getSubflow(id: string): any {
    return flow?.subflows?.[id]
  }

  useEffect(() => {
    let hash = window.location.hash.substr(1)
    // If no Flow ID given as the URL fragment
    // redirect to create a new Flow
    if (!hash) {
      setRedirect(true)
      return
    }

    // If Flow ID isn't set or if the Flow ID
    // from the URL fragment doesn't match the current state
    // then set the Flow ID to the given URL fragment
    if (flow.id === "" || flow.id !== hash) {
      handleFlow(hash)
      return
    }
  }, [])

  return (
    <React.Fragment>
      {flow?.name && <SEO title={flow?.name} />}
      <Columns centered={true} gapless={true}>
        <Columns.Column narrow={true} size={12}>
          {redirect && navigate("/studio/flow/new")}
          <Page
            subflow={getSubflow(subflowId)}
            // TODO create a React Context for handleElement and getSubflow
            handleElement={handleElement}
            handleVariable={handleVariable}
            getSubflow={getSubflow}
          ></Page>
        </Columns.Column>
      </Columns>
    </React.Fragment>
  )
}

export default Notebook
