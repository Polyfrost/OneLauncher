import { type IngressPayload, type IngressType, events } from '@onelauncher/client/bindings';
import { type Signal, createSignal, onMount } from 'solid-js';

type Ingresses = IngressPayload[];
type OnUpdateFn = (event: IngressType['type']) => unknown;

function useIngress(onUpdate?: OnUpdateFn): Signal<Ingresses> {
	const [ingresses, setIngresses] = createSignal<Ingresses>([]);

	async function update(event: IngressType['type']) {
		const ingress: Ingresses = []; // await manager.getNotifications();
		setIngresses(ingress);

		if (onUpdate)
			onUpdate(event);
	}

	onMount(() => {
		update('initialize');
		events.ingressPayload.listen(e => update(e.payload.event.type));
	});

	return [ingresses, setIngresses];
}

export default useIngress;
