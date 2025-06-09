import type { Context } from 'react';
import { createContext, useContext } from 'react';

const myContext = createContext(null) as Context<
	{
		text: string;
	} | null
>;

export function MyContextProvider({ children, value }: {
	children: React.ReactNode;
	value: {
		text: string;
	} | null;
}) {
	// eslint-disable-next-line react/no-context-provider -- ok
	return <myContext.Provider value={value}>{children}</myContext.Provider>;
}

export default function useMyContext() {
	const context = useContext(myContext);

	if (!context)
		throw new Error('useMyContext must be used within a MyContextProvider');

	return context;
}
