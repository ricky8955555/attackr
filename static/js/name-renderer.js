const predefinedColors = ['Crimson', 'PaleVioletRed', 'OrangeRed', 'Violet', 'BlueViolet', 'RebeccaPurple', 'Indigo', 'SlateBlue', 'MediumSeaGreen', 'CadetBlue', 'CornflowerBlue', 'DarkGoldenrod', 'Peru', 'Sienna', 'Brown', 'LightSlateGray', 'DarkSlateGray']

const calcTagColor = (tag, colors = null) => {
  const hashCode = s => s.split('').reduce((a, b) => (((a << 5) - a) + b.charCodeAt(0)) | 0, 0);

  colors = colors ?? predefinedColors;

  const idx = (hashCode(tag) >>> 0) % colors.length;
  const color = colors[idx];

  return color;
};

export const renderName = (element, colors = null) => {
  const tags = element.innerText.split('#');
  const raw = tags.shift();

  element.innerText = raw;

  for (const tag of tags) {
    const cur = document.createElement('span');

    const color = calcTagColor(tag.trim(), colors);
    cur.style.color = color;
    cur.innerText = `#${tag}`;

    element.appendChild(cur);
  };
};
