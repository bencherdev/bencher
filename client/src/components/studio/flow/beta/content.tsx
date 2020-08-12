import React from "react"
import Spinner from "react-spinkit"

// Content that dynamically updates itself
class DynamicContent extends React.Component {
  state = {
    paragraphs: this.props.children,
    spinner: true,
  }
  componentDidMount() {
    setTimeout(() => {
      this.setState({
        paragraphs: [
          ...this.state.paragraphs,
          `Once upon some other morning cheery, while I pondered, great and merry,\n
        Only this and nothing more.”`,
        ],
        spinner: false,
      })
      this.props.contentUpdated()
    }, 1200)
  }

  render() {
    const { title } = this.props,
      { paragraphs, spinner } = this.state

    return (
      <React.Fragment>
        <h3>{title}</h3>
        {paragraphs.map((c, i) => (
          <p key={i}>{c}</p>
        ))}
        <div>
          {spinner && (
            <Spinner
              name="cube-grid"
              color="green"
              fadeIn="none"
              style={{ margin: "0 auto" }}
            />
          )}
        </div>
      </React.Fragment>
    )
  }
}

export default DynamicContent
