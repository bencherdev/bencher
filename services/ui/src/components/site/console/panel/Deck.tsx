import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
} from "solid-js";

import Card from "./Card";
import DeckButton from "./DeckButton";

const Deck = (props) => {
  return (
    <>
      <DeckButton />

      <div class="columns">
        <div class="column">
          <div class="card">
            <Card field={"Project Name"} value={props?.data?.name} />
          </div>
        </div>
      </div>

      <div class="columns">
        <div class="column">
          <div class="card">
            <Card field={"Project Slug"} value={props?.data?.slug} />
          </div>
        </div>
      </div>

      <DeckButton />
    </>
  );
};

export default Deck;
