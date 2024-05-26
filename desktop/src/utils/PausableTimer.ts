export class PausableTimer {
	private _timeout: NodeJS.Timeout;
	private _paused: boolean = false;
	private _unixStarted: number = 0;
	private _unixStopped: number = 0;

	constructor(
		private readonly callback: () => unknown,
		private readonly duration: number,
		private readonly interval: boolean = false,
	) {
		this._timeout = this._createTimeout();
	}

	private _createTimeout(duration: number = this.duration): NodeJS.Timeout {
		this._unixStarted = Date.now();
		if (this.interval)
			return setInterval(this.callback, duration);
		else
			return setTimeout(this.callback, duration);
	}

	private _clearTimeout() {
		this._unixStopped = Date.now();
		if (this.interval)
			clearInterval(this._timeout);
		else
			clearTimeout(this._timeout);
	}

	public resume(restart: boolean = false) {
		const elapsed = this._unixStopped - this._unixStarted;
		const duration = this.interval || restart ? this.duration : this.duration - elapsed;

		this._paused = false;
		this._timeout = this._createTimeout(duration);
	}

	public pause() {
		this._clearTimeout();
		this._paused = true;
	}

	public stop() {
		this._clearTimeout();
	}

	public get timeout() {
		return this._timeout;
	}

	public get paused() {
		return this._paused;
	}
}

export default function createPausableTimer(callback: () => unknown, duration: number): PausableTimer {
	return new PausableTimer(callback, duration);
}
