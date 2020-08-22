import React, { useState, useEffect } from "react"
import { navigate } from "gatsby"
import { Container, Section, Heading, Columns } from "react-bulma-components"

import SEO from "../utils/seo"
import getFlow from "../studio/utils/getFlow"
import InterpreterWorker from "../../interpreter/interpreter"

const App = () => {
  const [flow, setFlow] = useState({
    id: "",
    main: "",
    name: "",
    subflows: {},
  })
  const [redirect, setRedirect] = useState(false)
  const [interpreter, setInterpreter] = useState()

  function handleFlow(id: string) {
    let currentFlow = getFlow(id)
    if (!currentFlow) {
      setRedirect(true)
    }
    let newFlow = { id: id, ...currentFlow }
    setFlow(newFlow)
    handleInterpreter(newFlow)
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
    <Container>
      {flow?.name && <SEO title={flow?.name} />}
      {redirect && navigate("/studio/flows")}
      <Section>
        <Heading size={1}>{flow?.name}</Heading>
      </Section>
      <Section>
        <Columns centered={true}>
          <Columns.Column size={12}>
            <Heading size={2}>Flow Input</Heading>
          </Columns.Column>
          <Columns.Column size={12}>
            <Heading size={2}>Flow Output</Heading>
          </Columns.Column>
        </Columns>
      </Section>
    </Container>
  )
}

export default App
