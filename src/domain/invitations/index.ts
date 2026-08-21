export interface IncomingInvite {
  id: number;
  uri: string;
}

export function normalizeInvite(value: string): string {
  const trimmed = value.trim();
  const fragment = trimmed.match(/^https:\/\/ideaflash\.cn\/#v1\/([^/?#\s]+)$/i);
  return fragment ? `sculk://join/v1/${fragment[1]}` : trimmed;
}

export function toWebInvite(value: string): string {
  const normalized = normalizeInvite(value);
  const payload = normalized.match(/^sculk:\/\/join\/v1\/([A-Za-z0-9_-]+)$/);
  return payload ? `https://ideaflash.cn/#v1/${payload[1]}` : normalized;
}

export function isSameInvite(left: string | null, right: string | null): boolean {
  return left !== null && right !== null && normalizeInvite(left) === normalizeInvite(right);
}

export function inviteFromDeepLinkUrls(urls: string[]): string | null {
  for (const url of urls) {
    const invite = normalizeInvite(url);
    if (invite.length <= 512 && /^sculk:\/\/join\/v1\/[A-Za-z0-9_-]+$/.test(invite)) {
      return invite;
    }
  }
  return null;
}
