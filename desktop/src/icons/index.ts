/**
 * Vendored provider marks from the MIT-licensed @lobehub/icons-static-svg
 * set (see desktop/LICENSES-THIRD-PARTY.md). All render with currentColor;
 * usage rules live in DESIGN.md "Provider identity".
 */
import anthropic from './vendor/anthropic.svg?raw';
import claude from './vendor/claude.svg?raw';
import cursor from './vendor/cursor.svg?raw';
import gemini from './vendor/gemini.svg?raw';
import githubcopilot from './vendor/githubcopilot.svg?raw';
import openai from './vendor/openai.svg?raw';
import xai from './vendor/xai.svg?raw';

const PROVIDER_MARKS: Record<string, string> = {
  anthropic,
  openai,
  google: gemini,
  github: githubcopilot,
  cursor,
  xai
};

const TOOL_MARKS: Record<string, string> = {
  'claude-code': claude,
  codex: openai,
  copilot: githubcopilot,
  cursor,
  gemini
};

export function providerMark(providerId: string): string | null {
  return PROVIDER_MARKS[providerId] ?? null;
}

export function toolMark(toolId: string): string | null {
  return TOOL_MARKS[toolId] ?? null;
}
