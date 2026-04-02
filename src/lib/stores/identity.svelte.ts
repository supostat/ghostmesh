import { getIdentity, type IdentityInfo } from "../api";

let identity = $state<IdentityInfo | null>(null);
let loading = $state(true);
let error = $state<string | null>(null);

export function getIdentityState() {
  return identity;
}

export function isIdentityLoading() {
  return loading;
}

export function getIdentityError() {
  return error;
}

export function setIdentity(value: IdentityInfo) {
  identity = value;
  loading = false;
  error = null;
}

export async function loadIdentity(): Promise<void> {
  loading = true;
  error = null;
  try {
    identity = await getIdentity();
  } catch (err) {
    identity = null;
    error = String(err);
  } finally {
    loading = false;
  }
}
