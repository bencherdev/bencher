import React, { useState, useEffect } from "react"
import { navigate } from "gatsby"
import { cloneDeep } from "lodash/lang"
import { Container, Columns } from "react-bulma-components"

import Toolbar from "./Toolbar"
import Page from "./Page"

import SEO from "../../utils/seo"
import getFlow from "../utils/getFlow"
import InterpreterWorker from "../../../interpreter/interpreter"

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
  const scale = 0.8

  const date = Date()

  function handleFlow(id: string) {
    let currentFlow = getFlow(id)
    if (!currentFlow) {
      setRedirect(true)
    }
    let newFlow = { id: id, ...currentFlow }
    setFlow(newFlow)
    setSubflowId(newFlow.main)
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
    <Container fluid={true} breakpoint="widescreen">
      {flow?.name && <SEO title={flow?.name} />}
      <Columns
        centered={true}
        gapless={true}
        style={{
          // https://stackoverflow.com/questions/10858523/css-transform-with-element-resizing
          transform: `scale(${scale})`,
          margin: `-550px calc((-550px * (1 - ${scale})) / 2) calc(-550px * (1 - ${scale}))`,
          zIndex: "-10",
          position: "static",
        }}
      >
        <Columns.Column size={12}>
          <Toolbar flowId={flow?.id} />
        </Columns.Column>
        <Columns.Column size={12}>
          <hr />
        </Columns.Column>
        <Columns.Column narrow={true} size={12}>
          {redirect && navigate("/studio/flows/new")}
          <Page
            subflow={getSubflow(subflowId)}
            // TODO create a React Context for handleElement and getSubflow
            handleElement={handleElement}
            handleVariable={handleVariable}
            getSubflow={getSubflow}
          />
        </Columns.Column>
      </Columns>
    </Container>
  )
}

export default Notebook
