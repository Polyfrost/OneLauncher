import type { ToOptions } from '@tanstack/react-router';
import { bindings } from '@/main';
import { useCommand } from '@onelauncher/common';
import { useLocation } from '@tanstack/react-router';
import { useEffect } from 'react';

type URLPath = Exclude<ToOptions['to'], undefined>;
export const ResolvedPathNames: Record<URLPath, string> = {
	'.': 'UNKNOWN',
	'..': 'UNKNOWN',
	'/': 'Viewing Home',
	'/app': 'Viewing Homepage',
	'/app/account': 'Viewing Account',
	'/app/account/skins': 'Viewing Skin Manager',
	'/app/cluster': 'Viewing Versions',
	'/app/cluster/browser': 'Viewing {clusterName}\'s mods',
	'/app/cluster/browser/package': 'Browsing {packageName}',
	'/app/cluster/logs': 'Viewing {clusterName}\'s logs',
	'/app/cluster/mods': 'Viewing {clusterName}\'s mods',
	'/app/cluster/resource-packs': 'Viewing {clusterName}\'s resource packs',
	'/app/cluster/shaders': 'Viewing {clusterName}\'s shaders',
	'/app/cluster/datapacks': 'Viewing {clusterName}\'s data packs',
	'/app/cluster/process': 'Viewing {clusterName}',
	'/app/cluster/settings': 'Viewing {clusterName}\'s settings',
	'/app/settings': 'Viewing Settings',
	'/app/settings/appearance': 'Viewing Settings',
	'/app/settings/developer': 'Viewing Settings',
	'/app/settings/minecraft': 'Viewing Settings',
	'/app/settings/changelog': 'Viewing Settings',
	'/app/accounts': 'Viewing Accounts',
	'/app/clusters': 'Viewing Versions',
	'/onboarding': 'Preparing OneClient',
	'/onboarding/account': 'Preparing OneClient',
	'/onboarding/finished': 'Preparing OneClient',
	'/onboarding/language': 'Preparing OneClient',
	'/onboarding/preferences/version': 'Preparing OneClient',
	'/onboarding/preferences/versionCategory': 'Preparing OneClient',
	'/onboarding/preferences': 'Preparing OneClient',
};

// Credit - https://github.com/DuckySoLucky/hypixel-discord-chat-bridge/blob/d3ea84a26ebf094c8191d50b4954549e2dd4dc7f/src/contracts/helperFunctions.js#L216-L225
function ReplaceVariables(template: string, variables: Record<string, any>) {
	return template.replace(/\{(\w+)\}/g, (match: any, name: string | number) => variables[name] ?? match);
}

export function useDiscordRPC() {
	const location = useLocation();
	const clusterId = location.search.clusterId ?? 0;
	const provider = location.search.provider ?? null;
	const packageId = location.search.packageId ?? null;
	const { data: cluster } = useCommand(['getClusterById', clusterId], () => bindings.core.getClusterById(clusterId));

	const { data: managedPackage } = useCommand(
		['getPackage', provider, packageId],
		() => {
			if (provider == null || packageId == null)
				return Promise.reject(new Error('Missing parameters'));
			return bindings.core.getPackage(provider, packageId);
		},
		{ enabled: provider != null && packageId != null },
	);

	useEffect(() => {
		const template = ResolvedPathNames[location.pathname as URLPath];
		if (template)
			bindings.core.setDiscordRPCMessage(ReplaceVariables(template, { clusterName: cluster?.name ?? 'UNKNOWN', packageName: managedPackage?.name ?? 'UNKNOWN' }));
	}, [location.pathname, location.search.clusterId, cluster?.name, managedPackage?.name]);
}
