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
    name: "Hello, Math!",
    // A map of all of the Subflows within a Flow
    subflows: {
      // A map of all Elements
      // and their order within their Subflow
      a1: {
        // The Subflow ID
        id: "a1",
        // The Subflows Parent Subflow ID
        parent: "",
        // The order of elements in the Subflow
        order: ["e1", "e2", "e3", "e4", "e0"],
        // Each Element is its own object
        elements: {
          // There will always be a Return element
          // even if it is unused
          e0: {
            // The id of the element
            id: "e0",
            // The type of the Element
            type: "return",
            // Each type will have a different value
            // TODO make these in Typescript
            value: {},
          },
          // Need a for Flow inputs
          e1: {
            id: "e1",
            type: "input",
            value: {},
          },
          e2: {
            id: "e2",
            type: "table",
            value: {
              name: "Input Table",
              var: "input_table",
              columns: [{ name: "Value", type: "Number" }],
              rows: [[5]],
            },
          },
          e3: {
            id: "e3",
            type: "function",
            value: {
              name: "Square",
              var: "square(Number)",
              params: [{ name: "Base", type: "Number", arg: "e2" }],
              returns: [{ name: "Result", type: "Number", arg: "e4" }],
            },
          },
          e4: {
            id: "e4",
            type: "table",
            value: {
              name: "Output Table",
              var: "output_table",
              columns: [
                {
                  name: "Squared Value",
                  type: "Number",
                },
              ],
              rows: [[25]],
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
  const [subflow, setSubflow] = useState("")
  const [redirect, setRedirect] = useState(false)
  const [interpreter, setInterpreter] = useState()

  const date = Date()

  function handleFlow(id: string) {
    let newFlow = { id: id, ...flows?.[id] }
    setFlow(newFlow)
    setSubflow(flows?.[id]?.main)
    handleInterpreter(newFlow)
  }

  function handleSubflow(id: string) {
    setSubflow(id)
  }

  function handleElement(element: any) {
    if (flow?.subflows?.[subflow]?.elements?.[element.id]?.value) {
      let newFlow = cloneDeep(flow)
      newFlow.subflows[subflow].elements[element.id].value = element
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

  useEffect(() => {
    let hash = window.location.hash
    // If no Flow ID given as the URL fragment
    // redirect to create a new Flow
    if (!hash) {
      setRedirect(true)
      return
    }

    hash = hash.replace("#", "")
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
      <Columns className="is-paddingless">
        <Columns.Column className="is-marginless">
          {redirect && navigate("/studio/flow/new")}
          <Page
            subflow={flow?.subflows?.[subflow]}
            handleElement={handleElement}
          ></Page>
        </Columns.Column>
      </Columns>
    </React.Fragment>
  )
}

export default Notebook
