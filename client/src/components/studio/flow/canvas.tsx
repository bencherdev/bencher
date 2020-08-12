import React, { useState, useRef } from "react"

import Flexbox from "../modeler/flexbox"
import { useInterval } from "../../utils/hooks/useInterval"
import Element from "../modeler/element"

const Canvas = (props: {
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
      <Flexbox
        style={{
          flexDirection: "row",
          justifyContent: "flex-start",
          flexWrap: "wrap",
          width: client.width,
          height: 1024,
        }}
      >
        <circle fill={"black"} r={50} />,
        <ellipse fill={"white"} rx={50} ry={30} />,
        <path d={"M 0 0 L 100 0 L 50 100 z"} fill={"orange"} />,
        <polygon fill={"cyan"} points={"0,100 50,0 100,100"} />,
        <text
          fill={"green"}
          fontFamily={"Arial,Helvetica"}
          style={{
            dominantBaseline: "text-before-edge",
          }}
        >
          {"wow such flexbox"}
        </text>
        ,
        <polyline
          fill={"none"}
          points={"0,100 50,0 100,100"}
          stroke={"yellow"}
          strokeWidth={3}
        />
        ,
        <rect fill={"red"} height={100} width={100} />,
        <image
          height={100}
          href={"https://media.giphy.com/media/RKCAeG662WQSc/giphy.gif"}
          width={80}
        />
        ,
        <rect fill={"#f0c"} height={10} width={10} />
        <rect fill={"#f0c"} height={10} width={20} />
      </Flexbox>
    </svg>
  )
}

export default Canvas
