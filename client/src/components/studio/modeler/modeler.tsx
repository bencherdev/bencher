import React, { useState, useEffect } from "react"

import Element from "./element"

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
            position: { x: 75, y: 150 },
            dimensions: { radius: 50 },
            value: {},
          },
          {
            // The type of the Element
            type: "table",
            position: { x: 250, y: 10 },
            dimensions: { width: 200, height: 250 },
            // Each type will have a different value
            // TODO make these in Typescript
            value: {
              title: "Input",
              var: "input",
              columns: [{ name: "Value", var: "value", type: "number" }],
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

const Modeler = (props: { path: string; id: string }) => {
  const [subflow, setSubflow] = useState({
    id: "",
    value: [],
  })
  const [nav, setNav] = useState("")

  let date = Date()

  function handleSubflow(id: string) {
    setNav(id)
    setSubflow({ id: id, value: flows[props.id]?.subflows?.[id] })
  }

  function handleNav(id: string) {
    handleSubflow(id)
  }

  useEffect(() => {
    if (subflow.id === "") {
      let main = flows[props.id]?.main
      handleSubflow(main)
    } else if (subflow.id !== nav) {
      handleNav(nav)
    }
  }, [])

  return (
    <div>
      <p>
        Modeler {props.path} {props.id} {date}
      </p>
      <svg width="100%" height="2000">
        {subflow &&
          subflow.value.map((line: any, lineIndex: number) => {
            return line.map((element: any, elementIndex: number) => {
              return (
                <Element
                  key={lineIndex.toString() + ":" + elementIndex.toString()}
                  prior={elementIndex === 0 ? null : line[elementIndex - 1]}
                  element={element}
                />
              )
            })
          })}
      </svg>
    </div>
  )
}

export default Modeler
