import { marked } from 'https://cdn.jsdelivr.net/npm/marked/+esm'
import DOMPurify from 'https://cdn.jsdelivr.net/npm/dompurify@3/+esm'

export const renderMarkdown = element => {
  const content = element.innerText;
  const html = DOMPurify.sanitize(marked.parse(content));
  element.innerHTML = html;
};
