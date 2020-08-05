import React from "react"
import { Link } from "@reach/router"

import SEO from "../../utils/seo"
import Flow from "./flow"

const flows: string[] = ["A", "B", "C"]

const Studio = () => {
  let date = Date()
  return (
    <div>
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
