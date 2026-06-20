// Shown while a Stripe checkout session is being created automatically, for
// example during onboarding with ?plan=pro. The visitor does not click
// anything, so this stands in for both the pricing table and the "Activate"
// button. The whole auto-activation becomes one calm loading state instead of
// a flash of intermediate UI before the redirect.
const CheckoutLoading = (props: { onboard?: boolean }) => {
	return (
		<div class="columns is-centered" style="margin-top: 1rem;">
			<div class={`column has-text-centered ${props.onboard ? "" : "is-half"}`}>
				<button
					class="button is-primary is-fullwidth is-loading"
					type="button"
					disabled
				>
					Redirecting to checkout
				</button>
				<p class="help">Taking you to secure checkout</p>
			</div>
		</div>
	);
};

export default CheckoutLoading;
