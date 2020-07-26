import React from "react"
import { Router } from "@reach/router"
import Loadable from "@loadable/component"

const Flow = Loadable(() => import("../../components/studio/flow"))

const flows: string[] = ["A", "B", "C"]

const Studio = (props: { path: string }) => {
  let date = Date()
  return (
    <div>
      <h1>TableFlow Studio</h1>
      {flows.map((flow, index) => {
        return <Flow key={index} flow={flow} />
      })}
    </div>
  )
}

export default Studio
