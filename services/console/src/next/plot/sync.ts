// uPlot keeps every sync key it is given for good, so stacked plots borrow keys from a pool instead of minting one per mount.

const free: string[] = [];
let made = 0;

/** A sync key no mounted plot holds. */
export const takeSyncKey = (): string => free.pop() ?? `plot-${++made}`;

export const returnSyncKey = (key: string): void => {
	free.push(key);
};
