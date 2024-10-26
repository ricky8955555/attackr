import { marked } from 'https://cdn.jsdelivr.net/npm/marked/+esm';
import DOMPurify from 'https://cdn.jsdelivr.net/npm/dompurify@3/+esm'

window.addEventListener('load', () => {
    const elements = document.getElementsByClassName('md-block');

    for (const element of elements) {
      const content = element.innerText;
      const html = DOMPurify.sanitize(marked.parse(content));
      element.innerHTML = html;
    }
});
