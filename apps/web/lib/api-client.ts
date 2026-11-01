import createClient from 'openapi-fetch';
import type { paths } from './api-schema';

/** Typed same-origin client; the server sets an HttpOnly scenario cookie. */
export const api = createClient<paths>({ baseUrl: '' });
