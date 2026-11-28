import { type Resource, Show } from "solid-js";
import { CardBrand, type JsonUsage } from "../../../../types/bencher";

const PaymentMethod = (props: { usage: Resource<null | JsonUsage> }) => {
	// Compared as text, so the generated enum stays out of every page's shared chunk.
	const collectionMethod = (): string | undefined =>
		props.usage()?.plan?.collection_method;
	return (
		<>
			<h4 class="title is-4">Payment Method</h4>
			<Show
				when={props.usage()?.plan?.card}
				fallback={
					<p>
						{collectionMethod() === "send_invoice"
							? "Invoice"
							: "Automatic payment"}
					</p>
				}
			>
				{(card) => (
					<>
						<CreditCard brand={card().brand} />
						<Show when={props.usage()?.plan?.customer?.name}>
							{(name) => <p>Name: {name()}</p>}
						</Show>
						<p>Last Four: {card().last_four}</p>
						<p>
							Expiration: {card().exp_month}/{card().exp_year - 2000}
						</p>
					</>
				)}
			</Show>
		</>
	);
};

const CreditCard = (props: { brand: undefined | CardBrand }) => {
	switch (props.brand) {
		case CardBrand.Amex: {
			return (
				<CreditCardBrand
					brand={brandedCard("cc-amex")}
					name="American Express"
				/>
			);
		}
		case CardBrand.Diners: {
			return (
				<CreditCardBrand
					brand={brandedCard("cc-diners-club")}
					name="Diners Club"
				/>
			);
		}
		case CardBrand.Discover: {
			return (
				<CreditCardBrand brand={brandedCard("cc-discover")} name="Discover" />
			);
		}
		case CardBrand.Jcb: {
			return <CreditCardBrand brand={brandedCard("cc-jcb")} name="JCB" />;
		}
		case CardBrand.Mastercard: {
			return (
				<CreditCardBrand
					brand={brandedCard("cc-mastercard")}
					name="Mastercard"
				/>
			);
		}
		case CardBrand.Unionpay: {
			return <CreditCardBrand brand={genericCard()} name="Unionpay" />;
		}
		case CardBrand.Visa: {
			return <CreditCardBrand brand={brandedCard("cc-visa")} name="Visa" />;
		}
		case CardBrand.Unknown: {
			return <CreditCardBrand brand={genericCard()} name="Credit Card" />;
		}
		default:
			return <CreditCardBrand brand={genericCard()} />;
	}
};

const CreditCardBrand = (props: { brand: string; name?: string }) => {
	return (
		<h4 class="subtitle is-4">
			<span class="icon-text">
				<span class="icon">
					<i class={props.brand} />
				</span>
				<span>{props.name}</span>
			</span>
		</h4>
	);
};

const brandedCard = (brand: string) => {
	return `fab fa-${brand}`;
};

const genericCard = () => {
	return "fas fa-credit-card";
};

export default PaymentMethod;
