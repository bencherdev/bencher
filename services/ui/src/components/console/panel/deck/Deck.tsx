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
      {props.config?.buttons && (
        <DeckButton
          pathname={props.pathname}
          handleRedirect={props.handleRedirect}
        />
      )}
      <For each={props.config?.cards}>
        {(card) => (
          <div class="columns">
            <div class="column">
              <div class="card">
                <Card
                  kind={card.kind}
                  field={card.field}
                  value={props.data?.[card.key]}
                />
              </div>
            </div>
          </div>
        )}
      </For>
      {props.config?.buttons && (
        <DeckButton
          pathname={props.pathname}
          handleRedirect={props.handleRedirect}
        />
      )}
    </>
  );
};

export default Deck;
