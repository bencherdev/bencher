import { createEffect } from "solid-js";

const PosterHeader = (props) => {
  createEffect(() => {
    const title = props.config?.title;
    if (title) {
      props.handleTitle(title);
    }
  });

  return (
    <nav class="level">
      <div class="level-left">
        <button
          class="button is-outlined"
          onClick={(e) => {
            e.preventDefault();
            props.handleRedirect(props.config?.path(props.pathname()));
          }}
        >
          <span class="icon">
            <i class="fas fa-chevron-left" aria-hidden="true" />
          </span>
          <span>Back</span>
        </button>
      </div>
      <div class="level-left">
        <div class="level-item">
          <h3 class="title is-3">{props.config?.title}</h3>
        </div>
      </div>

      <div class="level-right" />
    </nav>
  );
};

export default PosterHeader;
