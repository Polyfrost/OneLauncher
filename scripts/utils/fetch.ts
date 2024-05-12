import type { Agent } from 'undici';

const agentOptions: Agent.Options = {
	allowH2: true,
	connect: { timeout: CONNECT_TIMEOUT },
};
