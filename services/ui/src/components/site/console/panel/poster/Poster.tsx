import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
} from "solid-js";

const Poster = (props) => {
  return (
    <div class="columns">
      <div class="column">
        <div class="box">todo</div>
      </div>
    </div>
  );
};

export default Poster;
