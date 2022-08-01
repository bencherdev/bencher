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
      <DeckButton
        pathname={props.pathname}
        handleRedirect={props.handleRedirect}
      />
      <For each={props.config?.cards}>
        {(card) => (
          <div class="columns">
            <div class="column">
              <div class="card">
                <Card field={card.field} value={props.data?.[card.key]} />
              </div>
            </div>
          </div>
        )}
      </For>
      <DeckButton
        pathname={props.pathname}
        handleRedirect={props.handleRedirect}
      />
    </>
  );
};

export default Deck;
