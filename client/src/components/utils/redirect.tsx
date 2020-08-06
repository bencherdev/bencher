import React from "react"
import { navigate } from "gatsby"

// Client Side Only!
const Redirect = (props: { to: string }) => {
  return <React.Fragment>{navigate(props?.to)}</React.Fragment>
}

export default Redirect
