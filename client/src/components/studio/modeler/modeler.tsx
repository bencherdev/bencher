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
            position: { x: 75, y: 125 },
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
              name: "Input",
              var: "input",
              columns: [{ name: "Value", var: "value", type: "Number" }],
              rows: [[5]],
            },
          },
          {
            // The type of the Element
            type: "function",
            position: { x: 600, y: 30 },
            dimensions: { width: 200, height: 250 },
            // Each type will have a different value
            // TODO make these in Typescript
            value: {
              name: "Input",
              var: "input",
              columns: [{ name: "Value", var: "value", type: "Number" }],
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
  const [flow, setFlow] = useState({
    id: "",
    main: "",
    subflows: {},
  })
  const [subflow, setSubflow] = useState("")

  let date = Date()

  function handleFlow(id: string) {
    setFlow(flows?.[id])
    setSubflow(flows?.[id]?.main)
  }

  function handleSubflow(id: string) {
    setSubflow(id)
  }

  function handleElement(
    location: { line: number; position: number },
    element: any
  ) {
    if (
      flow?.subflows?.[subflow]?.[location.line]?.[location.position]?.value
    ) {
      let newFlow = JSON.parse(JSON.stringify(flow))
      newFlow.subflows[subflow][location.line][
        location.position
      ].value = element
      setFlow(newFlow)
    }
  }

  useEffect(() => {
    if (flow.id === "" || flow.id !== props.id) {
      handleSubflow("")
      handleFlow(props.id)
    }
  }, [])

  return (
    <div>
      <p>
        Modeler {props.path} {props.id} {date}
      </p>
      <svg width="100%" height="2000">
        {flow?.subflows?.[subflow] &&
          flow?.subflows?.[subflow]?.map((line: any, lineIndex: number) => {
            return line?.map((element: any, positionIndex: number) => {
              return (
                <Element
                  key={lineIndex.toString() + ":" + positionIndex.toString()}
                  location={{ line: lineIndex, position: positionIndex }}
                  prior={positionIndex === 0 ? null : line[positionIndex - 1]}
                  element={element}
                  handleElement={handleElement}
                />
              )
            })
          })}
      </svg>
    </div>
  )
}

export default Modeler
