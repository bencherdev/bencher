import { createContext, useContext } from "solid-js";
import type { JsonUser } from "../../types/bencher";
import type { Api } from "../api";
import type { Reader } from "../reader";

interface Account {
	api: Api;
	reader: () => Reader;
	/** The reader's own account as the API now has it. */
	keepUser: (user: JsonUser) => void;
	/** Sign the reader out. */
	signOut: () => void;
}

/** The API and the signed in reader, for the account pages. */
export const AccountContext = createContext<Account>();

export const useAccount = () => {
	const account = useContext(AccountContext);
	if (!account) {
		throw new Error("useAccount outside of the console's account pages");
	}
	return account;
};
