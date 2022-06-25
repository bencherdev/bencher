const SiteTextarea = (props) => {
  return (
    <textarea
      class="textarea"
      placeholder={props.config.placeholder}
      value={props.value}
      required
      onInput={(e) => props.handleField(e)}
    ></textarea>
  );
};

export default SiteTextarea;
