import React, { useState, useRef } from "react"

import { useInterval } from "../../utils/hooks/useInterval"
import Element from "../modeler/element"

const Page = (props: {
  canvas: { width: string; height: string }
  flow: any
  subflow: any
  handleElement: Function
}) => {
  const [client, setClient] = useState({
    width: 1024,
    height: 1024,
  })
  const svgRef = useRef(null)

  // Poll for a resized client window every second
  useInterval(() => {
    let width = svgRef?.current?.clientWidth
    let height = svgRef?.current?.clientHeight
    if (width != client.width || height != client.height) {
      setClient({
        width: width,
        height: height,
      })
    }
  }, 1000)

  return (
    <svg style={{ width: "100%", height: 1024 }} ref={svgRef}>
      {props?.flow?.subflows?.[props?.subflow]?.lines &&
        props?.flow?.subflows?.[props?.subflow]?.lines?.map(
          (line: any, lineIndex: number) => {
            // TODO break this into its own Line component
            // This component will keep state for the line
            // such as the midpoints, include when "wrap text" occurs
            return line?.map((elementId: any, positionIndex: number) => {
              let elements = props?.flow?.subflows?.[props?.subflow]?.elements
              return (
                <Element
                  key={lineIndex.toString() + ":" + positionIndex.toString()}
                  location={{ line: lineIndex, position: positionIndex }}
                  prior={
                    positionIndex === 0
                      ? null
                      : elements?.[line[positionIndex - 1]]
                  }
                  element={elements?.[elementId]}
                  handleElement={props.handleElement}
                />
              )
            })
          }
        )}
    </svg>
  )
}

export default Page
