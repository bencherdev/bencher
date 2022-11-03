const SiteHeaderPage = (props) => {
  props.handleTitle(props.page?.title);

  return (
    <div>
      <section class="section">
        <div class="container">
          <div class="columns is-mobile">
            <div class="column">
              <div class="content">
                <h2 class="title">{props.page.heading}</h2>
                <hr />
                <br />
                {props.page.content}
                <br />
              </div>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
};

export default SiteHeaderPage;
