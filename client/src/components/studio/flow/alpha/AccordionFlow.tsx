import React from "react"
import * as d3 from "d3"

class Row extends React.Component {
  state = {
    open: false,
    y: this.props.y,
  }

  rowRef = React.createRef()

  toggleOpen = () => this.setState({ open: !this.state.open })

  contentRefCallback = element => {
    if (element) {
      this.contentRef = element
      this.props.reportHeight(element.getBoundingClientRect().height)
    } else {
      this.props.reportHeight()
    }
  }

  contentUpdated = () => {
    this.props.reportHeight(this.contentRef.getBoundingClientRect().height)
  }

  componentDidUpdate() {
    const { y } = this.props
    d3.select(this.rowRef.current)
      .transition()
      .duration(500)
      .ease(d3.easeCubicInOut)
      .attr("transform", `translate(5, ${y})`)
      .on("end", () => {
        this.setState({
          y,
        })
      })
  }

  render() {
    const { icon, content, width } = this.props,
      { y } = this.state

    return (
      <g transform={`translate(5, ${y})`} ref={this.rowRef}>
        <g onClick={this.toggleOpen} style={{ cursor: "pointer" }}>
          {icon}
        </g>
        {this.state.open ? (
          <foreignObject
            x={20}
            y={-20}
            width={width}
            style={{ border: "1px solid red" }}
          >
            <div ref={this.contentRefCallback}>
              {typeof content === "function"
                ? content(this.contentUpdated)
                : content}
            </div>
          </foreignObject>
        ) : null}
      </g>
    )
  }
}

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
