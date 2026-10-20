// Markdown rendering moved to the client (X016, Q3), with the options the
// server used: `Str::markdown` (CommonMark + GFM tables/strikethrough),
// optionally with raw HTML stripped and unsafe links dropped.

import { Marked } from 'marked';

const UNSAFE = /^\s*(javascript|vbscript|file|data):/i;

const escapeHtml = (text) =>
  text.replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

function build({ stripHtml, safeLinks }) {
  const marked = new Marked({ gfm: true });
  marked.use({
    renderer: {
      html(token) {
        return stripHtml ? '' : token.text;
      },
    },
    walkTokens(token) {
      if (safeLinks && (token.type === 'link' || token.type === 'image') && UNSAFE.test(token.href)) {
        token.href = '';
      }
    },
  });
  return marked;
}

const unsafe = build({ stripHtml: false, safeLinks: false });
const safe = build({ stripHtml: true, safeLinks: true });

/** `Str::markdown($text)` (Jetstream's terms and policy pages). */
export const markdown = (text) => unsafe.parse(text ?? '');

/**
 * `Str::markdown($text, ['html_input' => 'strip', 'allow_unsafe_links' =>
 * false])` (journal post sections).
 */
export const safeMarkdown = (text) => safe.parse(text ?? '');

export { escapeHtml };
