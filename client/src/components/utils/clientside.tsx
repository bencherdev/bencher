import React from "react"
import { navigate } from "gatsby"

// Client Side Only!
const ClientSide = (props: { path: string }) => {
  return (
    <React.Fragment>
      <p>Client Side Only</p>
      <p>Given: {props.path}</p>
      <p>Actual: {window.location.href}</p>
    </React.Fragment>
  )
}

export default ClientSide
