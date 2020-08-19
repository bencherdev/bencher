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
        order: ["e0", "e3", "e5", "e7", "e1"],
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
              inputs: ["e2"],
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
              columns: [{ name: "Value", type: "Number" }],
              rows: [[5]],
            },
          },
          e3: {
            id: "e3",
            type: "decision",
            value: {
              name: "Decision",
              inputs: ["e2"],
              outputs: ["e4"],
              // TODO make this actually make sense
              columns: [
                {
                  name: "Decision Table",
                  type: "Number",
                },
              ],
              rows: [[25]],
            },
          },
          e4: {
            id: "e4",
            type: "table",
            value: {
              name: "Output Table",
              columns: [
                {
                  name: "Squared Value",
                  type: "Number",
                },
              ],
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
              name: "Output Table",
              columns: [
                {
                  name: "Squared Value",
                  type: "Number",
                },
              ],
              rows: [[25]],
            },
          },
          e7: {
            id: "e7",
            type: "subflow",
            value: {
              id: "a2",
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
              columns: [
                {
                  name: "Life",
                  type: "String",
                },
              ],
              rows: [["What is the meaning of life?"]],
            },
          },
          a2e4: {
            id: "a2e4",
            type: "table",
            value: {
              name: "The Answer",
              columns: [
                {
                  name: "Answer",
                  type: "Number",
                },
              ],
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

  function handleElement(id: string, value: any) {
    if (flow?.subflows?.[subflowId]?.elements?.[id]?.value) {
      let newFlow = cloneDeep(flow)
      newFlow.subflows[subflowId].elements[id].value = value
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
