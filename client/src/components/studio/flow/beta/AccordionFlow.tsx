import React from "react"
import Row from "./row"

class AccordionFlow extends React.Component {
  defaultHeight = 50
  state = {
    heights: this.props.data.map(_ => this.defaultHeight),
  }
  render() {
    const { data } = this.props,
      { heights } = this.state

    return (
      <g transform="translate(0, 20)">
        <line
          x1={15}
          x2={15}
          y1={10}
          y2={heights.reduce((sum, h) => sum + h, 0)}
          stroke="lightgrey"
          strokeWidth="2.5"
        />
        {data.map(([icon, content], i) => (
          <Row
            icon={icon}
            content={content}
            y={heights.slice(0, i).reduce((sum, h) => sum + h, 0)}
            width={450}
            height={heights[i]}
            key={i}
            reportHeight={height => {
              let tmp = [...heights]
              tmp[i] =
                height !== undefined && height > this.defaultHeight
                  ? height
                  : this.defaultHeight
              this.setState({ heights: tmp })
            }}
          />
        ))}
      </g>
    )
  }
}

export default AccordionFlow
