for (const expandable of document.querySelectorAll('.co-post-contents > .i-expandable')) {
    const cssState = /** @type {HTMLInputElement} */ expandable.querySelector('.i-expanded-state');
    let isOpen = cssState.checked;
    cssState.remove();

    const preview = expandable.querySelector('.i-post-body.i-preview');
    const full = expandable.querySelector('.i-post-body.i-full');

    const render = () => {
        if (isOpen && preview.parentNode) preview.remove();
        if (!isOpen && full.parentNode) full.remove();
        if (isOpen && !full.parentNode) expandable.append(full);
        if (!isOpen && !preview.parentNode) expandable.append(preview);
    };

    let readMore = expandable.querySelector('.i-post-body > .i-read-more');
    let readLess = expandable.querySelector('.i-post-body > .i-read-less');

    for (const readMoreReadLess of [readMore, readLess]) {
        const button = document.createElement('button');
        button.className = readMoreReadLess.className;
        button.textContent = readMoreReadLess.textContent;

        readMoreReadLess.replaceWith(button);

        if (readMoreReadLess === readMore) readMore = button;
        else readLess = button;
    }

    readMore.addEventListener('click', () => {
        isOpen = true;
        render();
    });
    readLess.addEventListener('click', () => {
        isOpen = false;
        const buttonFromTop = readLess.getBoundingClientRect().top;

        render();

        const newButtonPos = window.scrollY + readMore.getBoundingClientRect().top;
        window.scrollTo({ top: newButtonPos - buttonFromTop });
    });

    expandable.classList.remove('has-css-state');
    render();
}

for (const details of document.querySelectorAll('.co-themed-titled-box.large\\:expanded')) {
    const summary = details.querySelector('summary');

    let expanded = details.open;

    let wasLarge = window.innerWidth >= 1024;
    if (wasLarge) {
        details.open = true;
        details.classList.add('toggle-disabled');
    }

    const update = () => {
        const isLarge = window.innerWidth >= 1024;

        if (!wasLarge && isLarge) {
            details.open = true;
            details.classList.add('toggle-disabled');
        } else if (wasLarge && !isLarge) {
            details.open = expanded;
            details.classList.remove('toggle-disabled');
        }

        wasLarge = isLarge;
    };
    window.addEventListener('resize', update);

    details.addEventListener('toggle', (e) => {
        if (window.innerWidth >= 1024) {
            details.open = true;
            return;
        }

        expanded = details.open;
    });

    summary.addEventListener('click', (e) => {
        e.preventDefault();

        if (window.innerWidth >= 1024) return;

        if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) {
            details.open = !details.open;
            return;
        }

        const prevHeight = details.offsetHeight;
        details.open = !details.open;
        const newHeight = details.offsetHeight;

        summary.animate([
            { borderBottomLeftRadius: '0', borderBottomRightRadius: '0' },
            { borderBottomLeftRadius: '0', borderBottomRightRadius: '0' },
        ], { duration: 150 });

        details.animate([
            { height: `${prevHeight}px`, overflow: 'clip' },
            { height: `${newHeight}px`, overflow: 'clip' },
        ], {
            duration: 150,
            easing: 'cubic-bezier(0.4, 0, 0.2, 1)',
        });
    });
    summary.classList.add('can-animate');
}
