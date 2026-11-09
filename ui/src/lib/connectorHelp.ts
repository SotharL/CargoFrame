// Where a person gets what a connection asks for, so the field has a button to it. One list for
// all connectors: when a service moves a page, this is the only place to change.
import type { Connection } from './types';

export interface GetLinkSpec {
  label: string;
  url: string;
}

/** The fields a connection asks the person to fill from a service's own website. */
export type HelpField = 'token' | 'client_id' | 'client_secret' | 'webhook' | 'app_password';

/** The pages, by what they give. The forms' buttons and the guides both use these. */
export const PAGES = {
  jiraToken: { label: 'Create a token', url: 'https://id.atlassian.com/manage-profile/security/api-tokens' },
  googleCredentials: { label: 'Open Google credentials', url: 'https://console.cloud.google.com/apis/credentials' },
  redditApps: { label: 'Open Reddit apps', url: 'https://www.reddit.com/prefs/apps' },
  slackApps: { label: 'Open Slack apps', url: 'https://api.slack.com/apps' },
  discordWebhooks: { label: 'How to make one', url: 'https://support.discord.com/hc/en-us/articles/228383668-Intro-to-Webhooks' },
  discordPortal: { label: 'Open the developer portal', url: 'https://discord.com/developers/applications' },
  teamsApps: { label: 'Open app registrations', url: 'https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade' },
  xPortal: { label: 'Open the X developer portal', url: 'https://developer.x.com/en/portal/projects-and-apps' },
  blueskyPasswords: { label: 'Make an app password', url: 'https://bsky.app/settings/app-passwords' },
  metaApps: { label: 'Open my Meta apps', url: 'https://developers.facebook.com/apps/' },
  githubToken: { label: 'Make a token', url: 'https://github.com/settings/personal-access-tokens/new' },
} satisfies Record<string, GetLinkSpec>;

/**
 * The page where `field` of this connection is obtained, or `null` when there is none (a custom
 * API, or a Jira address that is not written yet).
 */
export function getLinkFor(c: Connection, field: HelpField): GetLinkSpec | null {
  switch (c.kind) {
    case 'jira': {
      if (field !== 'token') return null;
      if (c.auth === 'api_token') {
        return PAGES.jiraToken;
      }
      // Jira Server and Data Center: the token is made on the person's own profile page.
      const site = c.site_url.trim().replace(/\/+$/, '');
      return /^https:\/\/[^/\s]+/i.test(site)
        ? { label: 'Open my profile', url: `${site}/secure/ViewProfile.jspa` }
        : null;
    }
    case 'google':
      return field === 'client_id' || field === 'client_secret'
        ? PAGES.googleCredentials
        : null;
    case 'reddit':
      return field === 'client_id' ? PAGES.redditApps : null;
    case 'slack':
      return field === 'token' ? PAGES.slackApps : null;
    case 'discord':
      if (field === 'webhook') return PAGES.discordWebhooks;
      return field === 'token'
        ? PAGES.discordPortal
        : null;
    case 'teams':
      return field === 'client_id'
        ? PAGES.teamsApps
        : null;
    case 'x':
      return field === 'client_id' ? PAGES.xPortal : null;
    case 'bluesky':
      return field === 'app_password' ? PAGES.blueskyPasswords : null;
    case 'github':
      return field === 'token' ? PAGES.githubToken : null;
    case 'instagram':
      return field === 'token' ? PAGES.metaApps : null;
    default:
      return null;
  }
}

/** The Claude desktop app's download page, for a connection that opens it. */
export const CLAUDE_APP_DOWNLOAD = 'https://claude.ai/download';
