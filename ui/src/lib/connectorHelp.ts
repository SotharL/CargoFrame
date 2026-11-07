// Where a person gets what a connection asks for, so the field has a button to it. One list for
// all connectors: when a service moves a page, this is the only place to change.
import type { Connection } from './types';

export interface GetLinkSpec {
  label: string;
  url: string;
}

/** The fields a connection asks the person to fill from a service's own website. */
export type HelpField = 'token' | 'client_id' | 'client_secret' | 'webhook';

const DISCORD_WEBHOOKS = 'https://support.discord.com/hc/en-us/articles/228383668-Intro-to-Webhooks';

/**
 * The page where `field` of this connection is obtained, or `null` when there is none (a custom
 * API, or a Jira address that is not written yet).
 */
export function getLinkFor(c: Connection, field: HelpField): GetLinkSpec | null {
  switch (c.kind) {
    case 'jira': {
      if (field !== 'token') return null;
      if (c.auth === 'api_token') {
        return { label: 'Create a token', url: 'https://id.atlassian.com/manage-profile/security/api-tokens' };
      }
      // Jira Server and Data Center: the token is made on the person's own profile page.
      const site = c.site_url.trim().replace(/\/+$/, '');
      return /^https:\/\/[^/\s]+/i.test(site)
        ? { label: 'Open my profile', url: `${site}/secure/ViewProfile.jspa` }
        : null;
    }
    case 'google':
      return field === 'client_id' || field === 'client_secret'
        ? { label: 'Open Google credentials', url: 'https://console.cloud.google.com/apis/credentials' }
        : null;
    case 'reddit':
      return field === 'client_id' ? { label: 'Open Reddit apps', url: 'https://www.reddit.com/prefs/apps' } : null;
    case 'slack':
      return field === 'token' ? { label: 'Open Slack apps', url: 'https://api.slack.com/apps' } : null;
    case 'discord':
      if (field === 'webhook') return { label: 'How to make one', url: DISCORD_WEBHOOKS };
      return field === 'token'
        ? { label: 'Open the developer portal', url: 'https://discord.com/developers/applications' }
        : null;
    case 'teams':
      return field === 'client_id'
        ? { label: 'Open app registrations', url: 'https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade' }
        : null;
    default:
      return null;
  }
}

/** The Claude desktop app's download page, for a connection that opens it. */
export const CLAUDE_APP_DOWNLOAD = 'https://claude.ai/download';
