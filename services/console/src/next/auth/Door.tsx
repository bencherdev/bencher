import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Field from "@bencherdev/ui/Field";
import Heading from "@bencherdev/ui/Heading";
import TextInput from "@bencherdev/ui/TextInput";
import { Show, createSignal, onMount } from "solid-js";
import type { Failure } from "./copy";
import { GitHubMark, GoogleMark } from "./Frame";
import type { Door as DoorName } from "./pending";

export type Provider = "github" | "google";

const HEADINGS: Record<DoorName, string> = {
	login: "Sign in to Bencher",
	signup: "Sign up for Bencher",
};

export interface DoorProps {
	door: DoorName;
	github: boolean;
	google: boolean;
	/** The address filled in: an invite's, or the one Use another email took back. */
	email?: string | undefined;
	/** Selects the address filled in, as Use another email leaves it. */
	select?: boolean | undefined;
	busy?: boolean | undefined;
	failure?: Failure | undefined;
	onEmail?: (email: string) => void;
	onProvider?: (provider: Provider) => void;
	onRetry?: () => void;
}

/** One form that signs a person in or up: each provider the server has, then an email. */
const Door = (props: DoorProps) => {
	const [email, setEmail] = createSignal(props.email ?? "");
	let input: HTMLInputElement | undefined;
	onMount(() => {
		if (props.select) {
			input?.focus();
			input?.select();
		}
	});
	return (
		<>
			<Heading level={1} size="xl">
				{HEADINGS[props.door]}
			</Heading>
			<Show when={props.github || props.google}>
				<div class="au-stack">
					<Show when={props.github}>
						<Button
							size="lg"
							full
							disabled={props.busy}
							onClick={() => props.onProvider?.("github")}
						>
							<GitHubMark />
							Continue with GitHub
						</Button>
					</Show>
					<Show when={props.google}>
						<Button
							size="lg"
							full
							disabled={props.busy}
							onClick={() => props.onProvider?.("google")}
						>
							<GoogleMark />
							Continue with Google
						</Button>
					</Show>
				</div>
				<div class="au-or">or</div>
			</Show>
			{/* Posted, should a press beat the page's script, so the address never rides in a URL. */}
			<form
				class="au-stack"
				method="post"
				onSubmit={(event) => {
					event.preventDefault();
					props.onEmail?.(email().trim());
				}}
			>
				<Field label="Email" for="door-email">
					<TextInput
						ref={input}
						id="door-email"
						type="email"
						name="email"
						size="lg"
						autocomplete="email"
						required
						value={email()}
						onInput={(event) => setEmail(event.currentTarget.value)}
					/>
				</Field>
				<Button
					type="submit"
					variant="primary"
					size="lg"
					full
					disabled={props.busy}
				>
					Continue with email
				</Button>
				<Show when={props.failure}>
					{(failure) => (
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">{failure().text}</span>
							<Show when={failure().retry}>
								<Button size="sm" onClick={() => props.onRetry?.()}>
									Try again
								</Button>
							</Show>
						</Banner>
					)}
				</Show>
			</form>
			<p class="au-legal">
				By continuing, you agree to the{" "}
				<a href="/legal/terms-of-use" target="_blank" rel="noreferrer">
					Terms of Use
				</a>
				,{" "}
				<a href="/legal/privacy" target="_blank" rel="noreferrer">
					Privacy Policy
				</a>
				, and{" "}
				<a href="/legal/license" target="_blank" rel="noreferrer">
					License Agreement
				</a>
			</p>
		</>
	);
};

export default Door;
