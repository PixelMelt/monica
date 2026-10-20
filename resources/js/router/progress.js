// The progress bar Inertia showed during visits that take more than 250 ms
// (`progress: { color: '#4B5563' }`).

let bar = null;
let timer = null;
let trickle = null;
let value = 0;

function render() {
  if (!bar) {
    bar = document.createElement('div');
    bar.setAttribute('role', 'progressbar');
    Object.assign(bar.style, {
      position: 'fixed',
      top: '0',
      left: '0',
      height: '2px',
      background: '#4B5563',
      zIndex: '1031',
      transition: 'width 200ms ease, opacity 200ms ease',
    });
    document.body.appendChild(bar);
  }
  bar.style.opacity = '1';
  bar.style.width = `${value * 100}%`;
}

export default {
  start() {
    clearTimeout(timer);
    timer = setTimeout(() => {
      value = 0.08;
      render();
      trickle = setInterval(() => {
        value = Math.min(0.994, value + (1 - value) * 0.1);
        render();
      }, 200);
    }, 250);
  },
  done() {
    clearTimeout(timer);
    clearInterval(trickle);
    if (!bar || bar.style.opacity === '0') return;
    value = 1;
    render();
    setTimeout(() => {
      if (bar) bar.style.opacity = '0';
    }, 200);
  },
};
