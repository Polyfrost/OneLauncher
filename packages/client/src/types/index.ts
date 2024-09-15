import type { PROGRAM_INFO } from '../bindings';

export type ProgramInfo = typeof PROGRAM_INFO;
export type MakeOptional<T, K extends keyof T> = Omit<T, K> & Partial<Pick<T, K>>;
