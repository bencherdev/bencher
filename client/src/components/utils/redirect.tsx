import React from "react"
import { navigate } from "gatsby"

// Client Side Only!
const Redirect = (props: { to: string }) => {
  return <>{navigate(props?.to)}</>
}

export default Redirect
