import React, { useState, useEffect } from "react"

import Table from "./statictable"
import { number } from "prop-types"

const flows = {
  // Flow UUID
  a: {
    // The UUID for the main Flow in the Subflows
    main: "a1",
    // A map of all of the Subflows within a Flow
    subflows: {
      // A particular Subflow, may be the Main Flow
      a1: [
        // It is a list of lists
        // Think lines in a file for each Subflow
        [
          // Each Element is its own object
          // Need a New for new inputs
          {
            type: "new",
            value: {},
          },
          {
            // The type of the Element
            type: "table",
            // Each type will have a different value
            // TODO make these in Typescript
            value: {
              name: "Input",
              var: "input",
              columns: [{ name: "Input", var: "input", type: "int" }],
              rows: [[5]],
            },
          },
        ],
      ],
    },
  },
  b: null,
  c: null,
}

const Modeler = (props: { path: string; id: string; update: boolean }) => {
  const [subflow, setSubflow] = useState({
    id: "",
    value: [],
  })
  const [nav, setNav] = useState("")

  let date = Date()

  function handleSubflow(id: string) {
    setNav(id)
    console.log(flows[props.id]?.subflows?.[id])
    setSubflow({ id: id, value: flows[props.id]?.subflows?.[id] })
  }

  function handleNav(id: string) {
    handleSubflow(id)
  }

  function elementSwitch(element: any) {
    switch (element.type) {
      case "new":
        return <p>New Element</p>
      case "parent":
        return <p>Parent Flow</p>
      case "table":
        return <p>Table</p>
      case "decision":
        return <p>Decision Table</p>
      case "function":
        return <p>Function</p>
      case "subflow":
        return <p>Subflow</p>
      case "return":
        return <p>Return</p>
      default:
        return <p>Error: Unknown Element Type</p>
    }
  }

  useEffect(() => {
    if (subflow.id === "") {
      let main = flows[props.id]?.main
      handleSubflow(main)
      console.log("Main")
    } else if (subflow.id !== nav) {
      handleNav(nav)
      console.log("Updating")
    }
  }, [])

  return (
    <div>
      {console.log(subflow)}
      {subflow &&
        subflow.value.map((line: any, lineIndex: number) => {
          return line.map((element: any, elementIndex: number) => {
            return (
              <div key={lineIndex.toString() + ":" + elementIndex.toString()}>
                {elementSwitch(element)}
              </div>
            )
          })
        })}
      <p>
        Modeler {props.path} {props.id} {date}
      </p>
      <Table />
    </div>
  )
}

export default Modeler
