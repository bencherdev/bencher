import React from "react"
import { navigate } from "gatsby"

// Client Side Only!
const ClientSide = () => {
  return <p>Client Side Only: {window.location.href}</p>
}

export default ClientSide
