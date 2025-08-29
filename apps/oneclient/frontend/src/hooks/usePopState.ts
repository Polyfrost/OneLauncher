import { useLocation } from '@tanstack/react-router';
import { useEffect, useRef } from 'react';

export default function usePopState(onRouteChange: void | any) {
	const location = useLocation();
	const prevLocationRef = useRef(location);

	useEffect(() => {
		const prevLocation = prevLocationRef.current;

		// Only run if the pathname actually changed
		if (prevLocation.pathname !== location.pathname)
			onRouteChange?.({
				from: prevLocation,
				to: location,
			});

		prevLocationRef.current = location;
	}, [location, onRouteChange]);
}
