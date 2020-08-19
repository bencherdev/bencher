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
        order: ["e0", "e3", "e5", "e8", "e1"],
        // Each Element is its own object
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
              inputs: ["e2", "e7"],
            },
          },
          e1: {
            // There will always be a Output Element
            // even if it is unused
            id: "e1",
            type: "output",
            value: {
              outputs: ["e4"],
            },
          },
          e2: {
            id: "e2",
            type: "table",
            value: {
              name: "Input Table",
              columns: ["e2h1"],
              headers: {
                e2h1: { id: "e2h1", name: "Value", type: "Number" },
              },
              rows: [[5]],
            },
          },
          e3: {
            id: "e3",
            type: "decision",
            value: {
              name: "Square Formula",
              inputs: ["e2"],
              outputs: ["e4"],
              columns: {
                inputs: ["e3h1"],
                outputs: ["e3h2"],
              },
              headers: {
                inputs: {
                  e3h1: {
                    id: "e3h1",
                    table: "e2",
                    column: "e2h1",
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
                    table: "e4",
                    column: "e4h1",
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
          e4: {
            id: "e4",
            type: "table",
            value: {
              name: "Output Table",
              columns: ["e4h1"],
              headers: {
                e4h1: { id: "e4h1", name: "Squared Value", type: "Number" },
              },
              rows: [[25]],
            },
          },
          e5: {
            id: "e5",
            type: "function",
            value: {
              name: "Square",
              inputs: ["e4"],
              outputs: ["e6"],
            },
          },
          e6: {
            id: "e4",
            type: "table",
            value: {
              name: "Function Output Table",
              columns: ["e6h1"],
              headers: {
                e6h1: {
                  id: "e6h1",
                  name: "Function Squared Value",
                  type: "Number",
                },
              },
              rows: [[25]],
            },
          },
          e7: {
            id: "e7",
            type: "table",
            value: {
              name: "The Question",
              columns: ["e7h1"],
              headers: {
                e7h1: { id: "e7h1", name: "Life", type: "String" },
              },
              rows: [["What is the meaning of life?"]],
            },
          },
          e8: {
            id: "e8",
            type: "subflow",
            value: {
              id: "a2",
              inputs: ["e7"],
              outputs: ["e9"],
            },
          },
          e9: {
            id: "e9",
            type: "table",
            value: {
              name: "The Answer",
              columns: ["e9h1"],
              headers: {
                e9h1: { id: "e9h1", name: "Answer", type: "Number" },
              },
              rows: [[42]],
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
              inputs: ["a2e3"],
            },
          },
          a2e1: {
            id: "a2e1",
            type: "output",
            value: {
              outputs: ["a2e4"],
            },
          },
          a2e2: {
            id: "a2e2",
            type: "parent",
            value: {
              id: "a1",
            },
          },
          a2e3: {
            id: "a2e3",
            type: "table",
            value: {
              name: "The Question",
              columns: ["a2e3h1"],
              headers: {
                a2e3h1: { id: "a2e3h1", name: "Life", type: "String" },
              },
              rows: [["What is the meaning of life?"]],
            },
          },
          a2e4: {
            id: "a2e4",
            type: "table",
            value: {
              name: "The Answer",
              columns: ["a2e4h1"],
              headers: {
                a2e4h1: { id: "a2e4h1", name: "Answer", type: "Number" },
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
    let newFlow = { id: id, ...flows?.[id] }
    setFlow(newFlow)
    setSubflowId(flows?.[id]?.main)
    handleInterpreter(newFlow)
  }

  function handleSubflow(id: string) {
    setSubflowId(id)
  }

  function handleElement(
    id: string,
    value: any,
    elementSubflowId: string = ""
  ) {
    if (elementSubflowId === "") {
      elementSubflowId = subflowId
    }
    if (flow?.subflows?.[elementSubflowId]?.elements?.[id]?.value) {
      let newFlow = cloneDeep(flow)
      newFlow.subflows[elementSubflowId].elements[id].value = value
      setFlow(newFlow)
    }
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
      <SEO title={flow?.name} />
      <Columns centered={true} gapless={true}>
        <Columns.Column narrow={true} size={12}>
          {redirect && navigate("/studio/flow/new")}
          <Page
            subflow={getSubflow(subflowId)}
            // TODO create a React Context for handleElement and getSubflow
            handleElement={handleElement}
            getSubflow={getSubflow}
          ></Page>
        </Columns.Column>
      </Columns>
    </React.Fragment>
  )
}

export default Notebook
