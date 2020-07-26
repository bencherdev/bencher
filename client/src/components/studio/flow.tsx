import React from "react"
import { Link } from "@reach/router"

const Flow = (props: { flow: string }) => {
  return (
    <p>
      <Link to="/studio/flow/">Flow {props.flow}</Link>
    </p>
  )
}

export default Flow
