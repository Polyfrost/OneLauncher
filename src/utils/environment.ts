export function isDev() {
    return window.location.host.startsWith('localhost:');
}

export default {
    isDev,
};
