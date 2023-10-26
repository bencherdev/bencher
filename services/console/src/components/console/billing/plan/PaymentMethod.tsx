import type { Resource } from "solid-js";
import { CardBrand, type JsonUsage } from "../../../../types/bencher";

const PaymentMethod = (props: { usage: Resource<null | JsonUsage> }) => {
	return (
		<>
			<h4 class="title">Payment Method</h4>
			<CreditCard brand={props.usage()?.plan?.card?.brand} />
			<p>Name: {props.usage()?.plan?.customer?.name}</p>
			<p>Last Four: {props.usage()?.plan?.card?.last_four}</p>
			<p>
				Expiration: {props.usage()?.plan?.card?.exp_month}/
				{(props.usage()?.plan?.card?.exp_year ?? 2000) - 2000}
			</p>
		</>
	);
};

const CreditCard = (props: {
	brand: undefined | CardBrand;
}) => {
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
			return <CreditCardBrand brand={brandedCard("visa")} name="Visa" />;
		}
		case CardBrand.Unknown: {
			return <CreditCardBrand brand={genericCard()} name="Credit Card" />;
		}
		default:
			return <CreditCardBrand brand={genericCard()} />;
	}
};

const CreditCardBrand = (props: {
	brand: string;
	name?: string;
}) => {
	return (
		<h4 class="subtitle">
			<span class="icon-text">
				<span class="icon">
					<i class={props.brand} aria-hidden="true" />
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
