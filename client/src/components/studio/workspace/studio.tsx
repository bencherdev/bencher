import React, { useState, useEffect } from "react"
import { Link } from "@reach/router"

import SEO from "../../utils/seo"
import Flow from "./flow"

// TODO move to Modeler
import interpreter from "../../../workers/interpreter"
// TODO move to web worker
//import wasm from "../../../wasm/interpreter.bootstrap"
// import wasm from "../../../wasm/interpreter.wasm"
// import { greet } from "interpreter"
// const wasm = import("interpreter")
import Wasm from "../../../wasm/interpreter"

const flows: string[] = ["A", "B", "C"]

const Studio = () => {
  let date = Date()
  interpreter.run("life").then((result: any) => console.log(result))
  // wasm
  //   .then(instance => console.log(instance.greet()))
  //   .catch(err => console.log(err))
  return (
    <div>
      <Wasm />
      <SEO title="TableFlow Studio" />
      <h1>TableFlow Studio</h1>
      {flows.map((flow, index) => {
        return <Flow key={index} flow={flow} />
      })}
      <Link to="/studio/flow/new">New Flow</Link>
    </div>
  )
}

export default Studio
